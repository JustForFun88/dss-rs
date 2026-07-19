use super::*;

fn c(re: f64, im: f64) -> Complex64 {
    Complex64::new(re, im)
}

/// 2×2 complex system with a known solution.
#[test]
fn solves_small_dense_system() {
    let mut s = SparseSet::new(2);
    s.add_element(0, 0, c(4.0, 1.0));
    s.add_element(0, 1, c(-1.0, 0.0));
    s.add_element(1, 0, c(-1.0, 0.0));
    s.add_element(1, 1, c(3.0, -0.5));

    // Choose x, compute b = A·x, then recover x.
    let x_true = [c(1.0, 2.0), c(-0.5, 0.25)];
    let b = [
        c(4.0, 1.0) * x_true[0] + c(-1.0, 0.0) * x_true[1],
        c(-1.0, 0.0) * x_true[0] + c(3.0, -0.5) * x_true[1],
    ];
    let mut x = [Complex64::ZERO; 2];
    s.solve(&b, &mut x).unwrap();

    for (got, want) in x.iter().zip(&x_true) {
        assert!((got - want).norm() < 1e-12, "got {got}, want {want}");
    }
}

/// Duplicate triplets must accumulate.
#[test]
fn duplicate_entries_accumulate() {
    let mut s = SparseSet::new(1);
    s.add_element(0, 0, c(1.0, 0.0));
    s.add_element(0, 0, c(1.0, 0.0));
    let b = [c(4.0, 0.0)];
    let mut x = [Complex64::ZERO];
    s.solve(&b, &mut x).unwrap();
    assert!((x[0] - c(2.0, 0.0)).norm() < 1e-14);
}

/// Duplicate `(row, col)` stamps must be summed in **insertion order**, like
/// KLUSolve/CSparse `cs_dupl` — not faer's triplet-dedup order. f64 addition
/// isn't associative, so the order sets the last bit; matching it keeps the
/// assembled system Y bit-identical to the Pascal oracle on cells fed by several
/// elements. These three values are the real parts of the three line
/// contributions to `Y[632.1,632.1]` in IEEE13: summed in stamp order they give
/// the oracle's bits, a different order is 1 ULP off.
#[test]
fn duplicates_sum_in_insertion_order() {
    let (a, b, d) = (1.1451220523783028, 3.4336493324686748, 5.564622525570931);
    let mut s = SparseSet::new(1);
    s.add_element(0, 0, c(a, 0.0));
    s.add_element(0, 0, c(b, 0.0));
    s.add_element(0, 0, c(d, 0.0));
    let got = s.get_element(0, 0).unwrap().re;
    assert_eq!(
        got.to_bits(),
        ((a + b) + d).to_bits(),
        "must sum in stamp order"
    );
    // The order genuinely matters: stamping a,d,b instead is 1 ULP off.
    assert_ne!(((a + b) + d).to_bits(), ((a + d) + b).to_bits());
}

/// A structurally singular matrix (empty column) reports the column.
#[test]
fn singular_matrix_reports_column() {
    let mut s = SparseSet::new(2);
    s.add_element(0, 0, c(1.0, 0.0));
    // column/row 1 left empty -> singular
    let err = s.factor().unwrap_err();
    assert!(matches!(err, SparseError::Singular { .. }), "got {err:?}");
    assert!(s.singular_col().is_some());
}

/// `add_primitive_matrix` skips ground (node 0) and maps 1-based nodes.
#[test]
fn primitive_matrix_skips_ground() {
    // Two-node element between node 1 and ground: only (0,0) is stamped.
    let mut s = SparseSet::new(1);
    let y = c(10.0, -5.0);
    s.add_primitive_matrix(&[1, 0], &[y, -y, -y, y]);
    assert_eq!(s.nnz().unwrap(), 1);
    let b = [y * c(2.0, 0.0)];
    let mut x = [Complex64::ZERO];
    s.solve(&b, &mut x).unwrap();
    assert!((x[0] - c(2.0, 0.0)).norm() < 1e-12);
}

/// The cached-assemble fast path (P15 items 1-2: `zero()` + restamp the same
/// `(r,c)` pattern with new values) must reproduce a fresh from-scratch build
/// **bit-for-bit**. This is the bit-neutrality proof for the dedup-mapping cache
/// and its insertion-order re-accumulation — a stronger check than the
/// tolerance-based oracle comparison. The pattern has cells fed by several
/// stamps (the `(0,0)` diagonal), so the summation order actually matters.
#[test]
fn cached_fast_path_matches_fresh_bitwise() {
    // A 3×3 stamp pattern with duplicate (r,c) diagonal stamps + off-diagonals.
    let stamp = |s: &mut SparseSet, k: f64| {
        s.add_element(0, 0, c(1.1451220523783028 * k, 0.3 * k));
        s.add_element(0, 1, c(-k, 0.5 * k));
        s.add_element(1, 0, c(-k, 0.5 * k));
        s.add_element(0, 0, c(3.4336493324686748 * k, -0.2 * k)); // 2nd (0,0)
        s.add_element(1, 1, c(5.564622525570931 * k, 0.1 * k));
        s.add_element(2, 2, c(2.0 * k, -k));
        s.add_element(0, 0, c(5.564622525570931 * k, 0.7 * k)); // 3rd (0,0)
        s.add_element(1, 2, c(0.25 * k, 0.0 * k));
        s.add_element(2, 1, c(0.25 * k, 0.0 * k));
    };

    // Reused set: slow-path build, then zero()+restamp new values -> fast path.
    let mut reused = SparseSet::new(3);
    stamp(&mut reused, 1.0);
    let _ = reused.coo_entries().unwrap(); // force slow-path assemble + cache
    reused.zero();
    stamp(&mut reused, 2.0); // same pattern, different values -> fast path
    let (rr, rc, rv) = reused.coo_entries().unwrap();

    // Fresh set: the k=2.0 pattern from scratch (slow path only).
    let mut fresh = SparseSet::new(3);
    stamp(&mut fresh, 2.0);
    let (fr, fc, fv) = fresh.coo_entries().unwrap();

    assert_eq!((rr, rc), (fr, fc), "sparsity / CSC order must match");
    assert_eq!(rv.len(), fv.len());
    for (a, b) in rv.iter().zip(&fv) {
        assert_eq!(
            a.re.to_bits(),
            b.re.to_bits(),
            "re bits differ: {a:?} vs {b:?}"
        );
        assert_eq!(
            a.im.to_bits(),
            b.im.to_bits(),
            "im bits differ: {a:?} vs {b:?}"
        );
    }

    // And the factored solve agrees to the last bit too.
    let bvec = [c(1.0, -0.5), c(2.0, 0.3), c(-0.7, 1.1)];
    let mut xr = [Complex64::ZERO; 3];
    let mut xf = [Complex64::ZERO; 3];
    reused.solve(&bvec, &mut xr).unwrap();
    fresh.solve(&bvec, &mut xf).unwrap();
    for (a, b) in xr.iter().zip(&xf) {
        assert_eq!(a.re.to_bits(), b.re.to_bits(), "solve re bits differ");
        assert_eq!(a.im.to_bits(), b.im.to_bits(), "solve im bits differ");
    }
}

/// After `zero()`, restamping a **different** `(r,c)` pattern must invalidate the
/// cache (not silently reuse a stale skeleton) and produce the correct system.
#[test]
fn cache_invalidates_on_pattern_change() {
    let mut s = SparseSet::new(2);
    s.add_element(0, 0, c(2.0, 0.0));
    s.add_element(1, 1, c(2.0, 0.0));
    let _ = s.coo_entries().unwrap(); // cache the diagonal-only pattern
    assert_eq!(s.nnz().unwrap(), 2);

    s.zero();
    // New pattern: dense 2×2 (adds the two off-diagonals).
    s.add_element(0, 0, c(3.0, 0.0));
    s.add_element(0, 1, c(1.0, 0.0));
    s.add_element(1, 0, c(1.0, 0.0));
    s.add_element(1, 1, c(3.0, 0.0));
    assert_eq!(s.nnz().unwrap(), 4, "pattern change must be picked up");

    // [3 1; 1 3] x = [4; 4] -> x = [1; 1].
    let mut x = [Complex64::ZERO; 2];
    s.solve(&[c(4.0, 0.0), c(4.0, 0.0)], &mut x).unwrap();
    assert!((x[0] - c(1.0, 0.0)).norm() < 1e-12, "got {}", x[0]);
    assert!((x[1] - c(1.0, 0.0)).norm() < 1e-12, "got {}", x[1]);
}

/// A same-cell restamp keeps the `(r,c)` sequence identical (fast-path hit) but
/// carries new values in a new order — the re-accumulation must read the NEW
/// stamps in stamp order, never the cached values. Cached order a,b,d ->
/// (a+b)+d; restamped a,d,b -> (a+d)+b (a different last bit), so this fails
/// loudly if the fast path reused stale sums.
#[test]
fn cache_reaccumulates_new_values_not_stale() {
    let (a, b, d) = (1.1451220523783028, 3.4336493324686748, 5.564622525570931);
    let mut s = SparseSet::new(1);
    s.add_element(0, 0, c(a, 0.0));
    s.add_element(0, 0, c(b, 0.0));
    s.add_element(0, 0, c(d, 0.0));
    let cached = s.get_element(0, 0).unwrap().re; // slow-path build, order a,b,d
    assert_eq!(cached.to_bits(), ((a + b) + d).to_bits());
    s.zero();
    // Same single cell -> pattern matches -> fast path; new order a, d, b.
    s.add_element(0, 0, c(a, 0.0));
    s.add_element(0, 0, c(d, 0.0));
    s.add_element(0, 0, c(b, 0.0));
    let got = s.get_element(0, 0).unwrap().re;
    assert_eq!(
        got.to_bits(),
        ((a + d) + b).to_bits(),
        "fast path must re-sum new stamps"
    );
    assert_ne!(
        got.to_bits(),
        ((a + b) + d).to_bits(),
        "must not reuse the cached sum"
    );
}

/// P15 item 2 — the cached fast path must actually be *taken* on a same-pattern
/// `zero()`+restamp (not silently fall back to the from-scratch slow path, which
/// is bit-identical but pays the full HashMap dedup + faer rebuild). All the
/// bit-neutrality tests above pass equally on either path, so this asserts the
/// hot-path win itself via the `pattern_reused` fast-path sentinel: a cache-miss
/// regression that reverts to the slow path fails `cargo test`, not just a bench.
#[test]
fn same_pattern_rebuild_takes_fast_path() {
    let stamp = |s: &mut SparseSet, k: f64| {
        s.add_element(0, 0, c(2.0 * k, 0.1 * k));
        s.add_element(0, 1, c(-k, 0.0));
        s.add_element(1, 0, c(-k, 0.0));
        s.add_element(0, 0, c(1.0 * k, -0.2 * k)); // duplicate (0,0)
        s.add_element(1, 1, c(3.0 * k, 0.0));
    };
    let mut s = SparseSet::new(2);
    stamp(&mut s, 1.0);
    let _ = s.coo_entries().unwrap(); // slow-path build + cache
    assert!(
        !s.pattern_reused,
        "first assembly of a pattern must be the slow (from-scratch) build"
    );

    s.zero();
    stamp(&mut s, 2.0); // identical (r,c) sequence, new values
    let _ = s.coo_entries().unwrap();
    assert!(
        s.pattern_reused,
        "a same-pattern value-only rebuild must take the cached fast path"
    );

    // A pattern change must drop back to the slow path (cache invalidated).
    let mut s3 = SparseSet::new(3);
    s3.add_element(0, 0, c(2.0, 0.0));
    s3.add_element(1, 1, c(2.0, 0.0));
    s3.add_element(2, 2, c(2.0, 0.0));
    let _ = s3.coo_entries().unwrap();
    assert!(!s3.pattern_reused);
    s3.zero();
    // Different (r,c) sequence (adds an off-diagonal) -> must NOT reuse.
    s3.add_element(0, 0, c(2.0, 0.0));
    s3.add_element(0, 1, c(1.0, 0.0));
    s3.add_element(1, 1, c(2.0, 0.0));
    s3.add_element(2, 2, c(2.0, 0.0));
    let _ = s3.coo_entries().unwrap();
    assert!(
        !s3.pattern_reused,
        "a changed (r,c) stamp sequence must invalidate the cache (slow path)"
    );
}

/// P15 item 3 — `add_primitive_matrix_col_major` (the product's sole element
/// stamping path, `ymatrix.rs`) must be **bit-for-bit** equivalent to the old
/// `add_primitive_matrix(nodes, &cmatrix.to_row_major())`. Same `TcMatrix` read
/// column-major (element `(i,j)` at `cm[j*order+i]`) vs its row-major transpose
/// (`rm[i*order+j] == cm[j*order+i]`), stamped into two sets; the assembled COO
/// dumps must be identical to the last bit — the direct proof that the traversal
/// swap is index arithmetic only, not a reorder. Includes a ground node (0) to
/// exercise the row/col skip on both paths.
#[test]
fn col_major_stamp_matches_row_major_transpose_bitwise() {
    // Order-3 primitive with a ground node in the middle: nodes [1, 0, 2].
    let nodes = [1usize, 0, 2];
    let order = nodes.len();
    // Column-major storage (Pascal TcMatrix layout): cm[j*order + i] = elem(i,j).
    // Distinct, non-symmetric, irrational-ish values so a transpose slip shows.
    let elem = |i: usize, j: usize| c(1.0 + (i as f64) * 3.0 + (j as f64) * 7.0, 0.5 - j as f64);
    let mut cm = vec![Complex64::ZERO; order * order];
    let mut rm = vec![Complex64::ZERO; order * order];
    for i in 0..order {
        for j in 0..order {
            let v = elem(i, j);
            cm[j * order + i] = v; // column-major
            rm[i * order + j] = v; // row-major transpose
        }
    }

    let mut sc = SparseSet::new(2);
    sc.add_primitive_matrix_col_major(&nodes, &cm);
    let (rc, cc, vc) = sc.coo_entries().unwrap();

    let mut sr = SparseSet::new(2);
    sr.add_primitive_matrix(&nodes, &rm);
    let (rr, cr, vr) = sr.coo_entries().unwrap();

    assert_eq!((rc, cc), (rr, cr), "sparsity/order must match");
    assert_eq!(vc.len(), vr.len());
    for (a, b) in vc.iter().zip(&vr) {
        assert_eq!(
            a.re.to_bits(),
            b.re.to_bits(),
            "re bits differ: {a:?} vs {b:?}"
        );
        assert_eq!(
            a.im.to_bits(),
            b.im.to_bits(),
            "im bits differ: {a:?} vs {b:?}"
        );
    }
    // The ground node (nodes[1] == 0) contributes nothing: the two live nodes
    // (1, 2 -> matrix indices 0, 1) give a dense 2×2 = 4 nonzeros, not 3×3 = 9.
    assert_eq!(vc.len(), 4, "ground row/col must be skipped on both paths");
}

/// P15 item 6 — `find_islands`' recursion→iterative path-compression rewrite,
/// exercised for both **correctness** (connected components) and **scale** (the
/// degenerate long-chain the change targets: a chain that would drive the old
/// recursive `find` deep). The iterative version must return the right component
/// labels and complete without a stack overflow at chain scale.
#[test]
fn find_islands_components_and_deep_chain() {
    // Small mixed case: two islands {0,1,2} (a path) and {3,4} (an edge), plus an
    // isolated node {5}. Diagonals keep every column structurally nonzero.
    let mut s = SparseSet::new(6);
    for i in 0..6 {
        s.add_element(i, i, c(1.0, 0.0));
    }
    // island A: 0-1-2 chain
    s.add_element(0, 1, c(-1.0, 0.0));
    s.add_element(1, 0, c(-1.0, 0.0));
    s.add_element(1, 2, c(-1.0, 0.0));
    s.add_element(2, 1, c(-1.0, 0.0));
    // island B: 3-4 edge
    s.add_element(3, 4, c(-1.0, 0.0));
    s.add_element(4, 3, c(-1.0, 0.0));
    // node 5 isolated

    let comp = s.find_islands().unwrap();
    assert_eq!(comp[0], comp[1], "0,1 same island");
    assert_eq!(comp[1], comp[2], "1,2 same island");
    assert_eq!(comp[3], comp[4], "3,4 same island");
    assert_ne!(comp[0], comp[3], "the two islands must be distinct");
    assert_ne!(comp[0], comp[5], "isolated node is its own island");
    assert_ne!(comp[3], comp[5]);
    let distinct: std::collections::BTreeSet<_> = comp.iter().copied().collect();
    assert_eq!(distinct.len(), 3, "exactly three connected components");

    // Degenerate long chain 0-1-2-...-(N-1): a single island; the iterative
    // find/union must not overflow the stack at this scale (the stated reason
    // for the recursion→iteration rewrite).
    let n = 20_000;
    let mut chain = SparseSet::new(n);
    for i in 0..n {
        chain.add_element(i, i, c(1.0, 0.0));
    }
    for i in 0..n - 1 {
        chain.add_element(i, i + 1, c(-1.0, 0.0));
        chain.add_element(i + 1, i, c(-1.0, 0.0));
    }
    let comp = chain.find_islands().unwrap();
    let root = comp[0];
    assert!(
        comp.iter().all(|&c| c == root),
        "a fully-connected chain is one island"
    );
}

/// Refactoring after zero() + re-stamping works (the per-solve cycle
/// the DSS engine performs when Y changes).
#[test]
fn zero_and_rebuild() {
    let mut s = SparseSet::new(1);
    s.add_element(0, 0, c(2.0, 0.0));
    let mut x = [Complex64::ZERO];
    s.solve(&[c(2.0, 0.0)], &mut x).unwrap();
    assert!((x[0] - c(1.0, 0.0)).norm() < 1e-14);

    s.zero();
    s.add_element(0, 0, c(4.0, 0.0));
    s.solve(&[c(2.0, 0.0)], &mut x).unwrap();
    assert!((x[0] - c(0.5, 0.0)).norm() < 1e-14);
}
