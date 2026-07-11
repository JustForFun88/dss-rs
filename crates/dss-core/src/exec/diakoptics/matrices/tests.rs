//! Unit tests for the four A-Diakoptics matrix builders (`Calc_C_Matrix`,
//! `Calc_ZLL`, `Calc_ZCC`, `Calc_Y4`) with the D1 structural invariants
//! recomputed in-test and the D5 drop quirks asserted explicitly. Driven on a
//! tiny hand-checkable 3-phase feeder built inline (no file, no partitioner):
//! one `Line` acts as the "link" so the contour/ZLL/ZCC/Y4 algebra is exercised
//! end-to-end. The full-feeder fixture goldens land with the init machine.

use super::*;
use crate::support::sparse_math::SparseComplex;

/// Build a minimal solved 3-phase circuit: `sourcebus --link1--> b2 --load`.
fn solved_link_circuit() -> Dss {
    let mut dss = Dss::new();
    dss.command("clear");
    dss.command("new circuit.admat basekv=12.47");
    dss.command("new line.link1 phases=3 bus1=sourcebus bus2=b2 r1=0.1 x1=0.3 r0=0.2 x0=0.6 c1=0 c0=0 length=1 units=km");
    dss.command("new load.l1 phases=3 bus1=b2 kv=12.47 kw=1000 pf=0.9 model=1");
    dss.command("set voltagebases=[12.47]");
    dss.command("calcvoltagebases");
    dss.command("solve");
    assert!(dss.errors().is_empty(), "setup errors: {:?}", dss.errors());
    assert!(
        dss.circuit().is_some_and(|c| c.solution.converged_flag),
        "base circuit failed to converge"
    );
    dss
}

/// `Link_Branches` shape: index-0 empty placeholder + the real link(s).
fn links_with_placeholder(names: &[&str]) -> Vec<String> {
    let mut v = vec![String::new()];
    v.extend(names.iter().map(|s| s.to_string()));
    v
}

/// A dense row-major `n×n` copy of a sparse-complex matrix.
fn to_dense(m: &SparseComplex, n: usize) -> Vec<Complex64> {
    let mut d = vec![Complex64::ZERO; n * n];
    for cd in &m.cdata {
        let (r, c) = (cd.row as usize, cd.col as usize);
        if r < n && c < n {
            d[r * n + c] = cd.value;
        }
    }
    d
}

fn dense_mul(a: &[Complex64], b: &[Complex64], n: usize) -> Vec<Complex64> {
    let mut out = vec![Complex64::ZERO; n * n];
    for i in 0..n {
        for j in 0..n {
            let mut s = Complex64::ZERO;
            for k in 0..n {
                s += a[i * n + k] * b[k * n + j];
            }
            out[i * n + j] = s;
        }
    }
    out
}

#[test]
fn contours_have_one_plus_and_one_minus_per_column() {
    let mut dss = solved_link_circuit();
    let links = links_with_placeholder(&["line.link1"]);
    let rc = dss.calc_c_matrix(&links);
    assert_eq!(rc, 0, "Calc_C_Matrix result code");

    let ckt = dss.circuit().unwrap();
    let contours = &ckt.ad.contours;
    // 1 real link × 3 phases = 3 contour columns.
    assert_eq!(contours.ncols(), 3, "contour columns");
    assert_eq!(contours.nzero(), 6, "two entries per column");

    // D1 invariant: each column has exactly one +1 and one -1.
    for col in 0..3 {
        let entries: Vec<_> = contours.cdata.iter().filter(|cd| cd.col == col).collect();
        assert_eq!(entries.len(), 2, "column {col} entry count");
        let plus = entries
            .iter()
            .filter(|cd| cd.value == Complex64::new(1.0, 0.0))
            .count();
        let minus = entries
            .iter()
            .filter(|cd| cd.value == Complex64::new(-1.0, 0.0))
            .count();
        assert_eq!(plus, 1, "column {col}: one +1");
        assert_eq!(minus, 1, "column {col}: one -1");
        // The two boundary nodes are distinct.
        assert_ne!(
            entries[0].row, entries[1].row,
            "column {col}: distinct nodes"
        );
    }
}

#[test]
fn calc_c_matrix_rejects_non_line_link() {
    let mut dss = solved_link_circuit();
    // A transformer-shaped link name → result -1 (Diakoptics.pas:395).
    let links = links_with_placeholder(&["transformer.tx"]);
    assert_eq!(dss.calc_c_matrix(&links), -1);
}

#[test]
fn zll_block_is_inverted_link_yprim_self_block() {
    let mut dss = solved_link_circuit();
    let links = links_with_placeholder(&["line.link1"]);
    dss.calc_c_matrix(&links);
    let rc = dss.calc_zll(&links);
    assert_eq!(rc, 0, "Calc_ZLL result code");

    // Independently extract the link's 3×3 Yprim self-block and invert it.
    let mut expected = {
        let elem = ad_find_element(dss.registered_classes(), "line.link1").unwrap();
        let yprim = elem.cd().yprim.as_ref().unwrap();
        let v = yprim.values();
        let mut lp = CMatrix::new(3);
        // Same k-walk as Calc_ZLL (top-left 3×3 of the 6-wide Yprim).
        let (mut k, mut row, mut col, mut count) = (1usize, 1usize, 1usize, 0usize);
        for _ in 0..9 {
            lp.set(row - 1, col - 1, v[k - 1]);
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
        lp
    };
    expected.invert().unwrap();

    let ckt = dss.circuit().unwrap();
    let zll = &ckt.ad.zll;
    // D1 invariant: ZLL is 3×3 (single link), block-diagonal at offset 0, and
    // equals the inverted self-block placed row-major.
    assert_eq!(zll.nzero(), 9, "3×3 dense block");
    for cd in &zll.cdata {
        assert!(
            cd.row < 3 && cd.col < 3,
            "entry inside the single 3×3 block"
        );
        let e = expected.get(cd.row as usize, cd.col as usize);
        let diff = (cd.value - e).norm();
        assert!(
            diff < 1e-9,
            "ZLL[{},{}] {} vs {}",
            cd.row,
            cd.col,
            cd.value,
            e
        );
    }
}

#[test]
fn y4_is_zcc_inverse_modulo_d5_drops() {
    let mut dss = solved_link_circuit();
    let links = links_with_placeholder(&["line.link1"]);
    assert_eq!(dss.calc_c_matrix(&links), 0);
    assert_eq!(dss.calc_zll(&links), 0);
    dss.calc_zcc(links.len());
    dss.calc_y4();

    let ckt = dss.circuit().unwrap();
    let n = ckt.ad.zcc.nrows() as usize;
    assert_eq!(n, 3, "ZCC is (real-links × 3) square");
    // ZCC = Contoursᵀ·ZCT + ZLL is square with the same order.
    assert_eq!(ckt.ad.zcc.ncols() as usize, n);

    // D1 invariant: Y4·ZCC ≈ I (dense; avoids the SparseComplex.multiply drop
    // quirk so we test the invert itself). The D5 Y4 drop only removes entries
    // with re == 0, of which a real impedance inverse has none — so the dense
    // product is the identity to invert precision.
    let y4 = to_dense(&ckt.ad.y4, n);
    let zcc = to_dense(&ckt.ad.zcc, n);
    let prod = dense_mul(&y4, &zcc, n);
    for i in 0..n {
        for j in 0..n {
            let want = if i == j { 1.0 } else { 0.0 };
            let got = prod[i * n + j];
            assert!(
                (got.re - want).abs() < 1e-7 && got.im.abs() < 1e-7,
                "Y4·ZCC[{i},{j}] = {got} (want {want})"
            );
        }
    }

    // D5 drop pattern asserted explicitly: every STORED Y4 entry has re ≠ 0
    // (the `re<>0 AND re<>0` test keeps only nonzero-real entries; a pure-
    // imaginary inverse entry would be dropped — none arise here).
    for cd in &ckt.ad.y4.cdata {
        assert_ne!(cd.value.re, 0.0, "Y4 stores only nonzero-real entries (D5)");
    }
}
