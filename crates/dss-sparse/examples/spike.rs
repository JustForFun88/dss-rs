//! Phase 0 spike: is faer's complex sparse LU adequate for DSS-sized Y matrices?
//!
//! Builds a synthetic admittance-like matrix on a 2D grid graph (each node
//! coupled to up to 4 neighbors — denser than a real distribution feeder,
//! which is mostly radial, so this is a pessimistic case), then times:
//! CSC assembly, symbolic analysis, numeric factorization, numeric
//! REfactorization with reused symbolic (the per-control-iteration cycle in
//! OpenDSS), and the triangular solve. Verifies the residual ‖Ax−b‖∞.
//!
//! Run: cargo run --release -p dss-sparse --example spike

use std::time::Instant;

use faer::MatMut;
use faer::linalg::solvers::Solve;
use faer::prelude::Reborrow;
use faer::sparse::linalg::solvers::{Lu, SymbolicLu};
use faer::sparse::{SparseColMat, Triplet};
use num_complex::Complex64;

type Trip = Triplet<usize, usize, Complex64>;

/// Deterministic LCG so the spike needs no `rand` dependency.
struct Lcg(u64);

impl Lcg {
    fn next_f64(&mut self) -> f64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Branch admittance with realistic distribution-feeder magnitudes:
/// y = 1/(r + jx), r ∈ [0.01, 0.11) Ω, x ∈ [0.05, 0.35) Ω.
fn branch_admittance(rng: &mut Lcg) -> Complex64 {
    let r = 0.01 + 0.1 * rng.next_f64();
    let x = 0.05 + 0.3 * rng.next_f64();
    Complex64::new(1.0, 0.0) / Complex64::new(r, x)
}

/// Y-bus-like triplets for a side×side grid graph.
fn build_grid_triplets(side: usize, seed: u64) -> Vec<Trip> {
    let n = side * side;
    let mut rng = Lcg(seed);
    let mut triplets: Vec<Trip> = Vec::with_capacity(5 * n);
    let mut diag = vec![Complex64::ZERO; n];

    for row in 0..side {
        for col in 0..side {
            let node = row * side + col;
            let mut stamp = |a: usize, b: usize, y: Complex64, diag: &mut [Complex64]| {
                triplets.push(Triplet::new(a, b, -y));
                triplets.push(Triplet::new(b, a, -y));
                diag[a] += y;
                diag[b] += y;
            };
            if col + 1 < side {
                let y = branch_admittance(&mut rng);
                stamp(node, node + 1, y, &mut diag);
            }
            if row + 1 < side {
                let y = branch_admittance(&mut rng);
                stamp(node, node + side, y, &mut diag);
            }
        }
    }
    // Shunt term keeps the matrix nonsingular (line charging to ground, plus
    // a few strongly grounded "source" nodes, like substation buses).
    for (i, d) in diag.iter().enumerate() {
        let shunt = if i % 997 == 0 {
            Complex64::new(100.0, 0.0)
        } else {
            Complex64::new(1e-4, 1e-3)
        };
        triplets.push(Triplet::new(i, i, d + shunt));
    }
    triplets
}

fn max_residual(triplets: &[Trip], x: &[Complex64], b: &[Complex64]) -> f64 {
    let mut ax = vec![Complex64::ZERO; b.len()];
    for t in triplets {
        ax[t.row] += t.val * x[t.col];
    }
    ax.iter()
        .zip(b)
        .map(|(l, r)| (l - r).norm())
        .fold(0.0, f64::max)
}

fn run_case(side: usize) {
    let n = side * side;
    println!("--- {n} nodes ({side}x{side} grid) ---");

    let t = Instant::now();
    let triplets = build_grid_triplets(side, 0x5DEECE66D);
    println!(
        "  triplet build:       {:>10.3} ms",
        t.elapsed().as_secs_f64() * 1e3
    );

    let t = Instant::now();
    let matrix =
        SparseColMat::<usize, Complex64>::try_new_from_triplets(n, n, &triplets).expect("assemble");
    println!(
        "  assemble (CSC):      {:>10.3} ms   nnz = {}",
        t.elapsed().as_secs_f64() * 1e3,
        matrix.compute_nnz()
    );

    let t = Instant::now();
    let symbolic = SymbolicLu::try_new(matrix.symbolic()).expect("symbolic");
    println!(
        "  symbolic analysis:   {:>10.3} ms",
        t.elapsed().as_secs_f64() * 1e3
    );

    let t = Instant::now();
    let lu = Lu::try_new_with_symbolic(symbolic.clone(), matrix.rb()).expect("numeric");
    println!(
        "  numeric factor:      {:>10.3} ms",
        t.elapsed().as_secs_f64() * 1e3
    );

    // Refactor with new values, same pattern — the cycle OpenDSS runs when a
    // tap changes or a control acts (KLU's refactor path).
    let triplets2 = build_grid_triplets(side, 0xDEADBEEF);
    let matrix2 = SparseColMat::<usize, Complex64>::try_new_from_triplets(n, n, &triplets2)
        .expect("assemble2");
    let t = Instant::now();
    let lu2 = Lu::try_new_with_symbolic(symbolic, matrix2.rb()).expect("refactor");
    println!(
        "  refactor (reuse sym):{:>10.3} ms",
        t.elapsed().as_secs_f64() * 1e3
    );

    let mut rng = Lcg(42);
    let b: Vec<Complex64> = (0..n)
        .map(|_| Complex64::new(rng.next_f64() - 0.5, rng.next_f64() - 0.5))
        .collect();
    let mut x = b.clone();

    let t = Instant::now();
    lu.solve_in_place(MatMut::from_column_major_slice_mut(&mut x, n, 1));
    println!(
        "  solve (1 rhs):       {:>10.3} ms",
        t.elapsed().as_secs_f64() * 1e3
    );
    println!(
        "  residual ‖Ax−b‖∞:   {:>10.3e}",
        max_residual(&triplets, &x, &b)
    );

    let mut x2 = b.clone();
    lu2.solve_in_place(MatMut::from_column_major_slice_mut(&mut x2, n, 1));
    println!(
        "  residual (refactor): {:>9.3e}",
        max_residual(&triplets2, &x2, &b)
    );
}

fn main() {
    // ~2.5k, 10k, 25k, 90k nodes.
    for side in [50usize, 100, 158, 300] {
        run_case(side);
    }
}
