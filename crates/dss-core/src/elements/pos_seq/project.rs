//! Dense linear algebra of the positive-sequence reduction: a complex LU
//! solve with partial pivoting, the inverse built on it, and the passivity
//! test of a real conductance matrix. The arithmetic is plain `num_complex`
//! and `f64`, the same in both lanes.

use crate::support::cmatrix::CMatrix;
use num_complex::Complex64;

/// The pivot size at or below which [`lu_solve`] calls `a` singular:
/// `n · EPSILON · max |a_ij|`, with `n` the order. A pivot that low leaves a
/// condition number of about `1 / (n · EPSILON)`, where `f64` carries no digit
/// of the solution.
fn singular_below(a: &CMatrix) -> f64 {
    let scale = a.values().iter().fold(0.0_f64, |m, v| m.max(v.norm()));
    a.order() as f64 * f64::EPSILON * scale
}

/// The LU factors of a square matrix, row-major: the unit lower `L` below
/// the diagonal, `U` on and above it, and the row each step swapped in.
struct Factors {
    n: usize,
    lu: Vec<Complex64>,
    swaps: Vec<usize>,
    /// The steps completed. Fewer than `n` when the pivot of step `steps`
    /// (left in `lu` on the diagonal) lies at or below [`singular_below`].
    steps: usize,
}

/// Doolittle factorization of `a` with partial pivoting on `norm()`. It stops
/// at the first pivot at or below [`singular_below`], or that is not a
/// number, which an elimination that overflows leaves (`∞ - ∞`).
fn factor(a: &CMatrix) -> Factors {
    let n = a.order();
    let tiny = singular_below(a);
    let mut lu = a.to_row_major();
    let mut swaps = Vec::with_capacity(n);
    for k in 0..n {
        let mut p = k;
        for i in k + 1..n {
            if lu[i * n + k].norm() > lu[p * n + k].norm() {
                p = i;
            }
        }
        if p != k {
            for j in 0..n {
                lu.swap(k * n + j, p * n + j);
            }
        }
        swaps.push(p);
        let pivot = lu[k * n + k];
        let size = pivot.norm();
        if size.is_nan() || size <= tiny {
            return Factors {
                n,
                lu,
                swaps,
                steps: k,
            };
        }
        for i in k + 1..n {
            let l = lu[i * n + k] / pivot;
            lu[i * n + k] = l;
            for j in k + 1..n {
                let u = lu[k * n + j];
                lu[i * n + j] -= l * u;
            }
        }
    }
    Factors {
        n,
        lu,
        swaps,
        steps: n,
    }
}

impl Factors {
    /// `x` with `A x = b`, one column of `x` per column of `b`: the row swaps
    /// applied to `b`, then forward substitution with `L` and back
    /// substitution with `U`, column by column.
    fn solve(&self, b: &CMatrix) -> CMatrix {
        let n = self.n;
        let mut rhs = b.to_row_major();
        for (k, &p) in self.swaps.iter().enumerate() {
            if p != k {
                for c in 0..n {
                    rhs.swap(k * n + c, p * n + c);
                }
            }
        }
        let mut x = CMatrix::new(n);
        for c in 0..n {
            let mut y: Vec<Complex64> = (0..n).map(|i| rhs[i * n + c]).collect();
            for i in 0..n {
                let lower = &self.lu[i * n..i * n + i];
                y[i] = lower.iter().zip(&y[..i]).fold(y[i], |s, (l, v)| s - l * v);
            }
            for i in (0..n).rev() {
                let upper = &self.lu[i * n + i + 1..(i + 1) * n];
                let s = upper
                    .iter()
                    .zip(&y[i + 1..])
                    .fold(y[i], |s, (u, v)| s - u * v);
                y[i] = s / self.lu[i * n + i];
            }
            for (i, v) in y.into_iter().enumerate() {
                x.set(i, c, v);
            }
        }
        x
    }
}

/// `x` with `a x = b`, one column of `x` per column of `b`, by an LU
/// factorization with partial pivoting. `None` when an entry of `a` or `b` is
/// not finite, since such a system has no solution to report, and when a
/// pivot is zero at the matrix's own scale (at or below
/// `n · EPSILON · max |a_ij|`) or not a number.
pub fn lu_solve(a: &CMatrix, b: &CMatrix) -> Option<CMatrix> {
    assert_eq!(
        a.order(),
        b.order(),
        "lu_solve: the right-hand side must have the order of the matrix"
    );
    if a.values().iter().chain(b.values()).any(|v| !v.is_finite()) {
        return None;
    }
    let f = factor(a);
    (f.steps == f.n).then(|| f.solve(b))
}

/// The inverse of `a`: [`lu_solve`] against the identity.
pub fn inverse(a: &CMatrix) -> Option<CMatrix> {
    let n = a.order();
    let mut identity = CMatrix::new(n);
    for i in 0..n {
        identity.set(i, i, Complex64::ONE);
    }
    lu_solve(a, &identity)
}

/// The smallest eigenvalue of the symmetric part `(G + Gᵀ) / 2` of the real
/// `n × n` matrix `g` (row-major), by cyclic Jacobi rotations on that part
/// divided by its largest entry magnitude, so that no square overflows or
/// underflows at any scale. The sweeps stop when the off-diagonal Frobenius
/// norm is at or below `1e-15` of the Frobenius norm, and after 100 sweeps at
/// most. A symmetric part of zeros reads 0, an empty matrix has no eigenvalue
/// and reads `+∞`, and a matrix with an entry that is not finite has none and
/// reads NaN.
pub fn smallest_symmetric_eigenvalue(g: &[f64], n: usize) -> f64 {
    assert_eq!(g.len(), n * n, "a {n} x {n} matrix has {} entries", n * n);
    if g.iter().any(|v| !v.is_finite()) {
        return f64::NAN;
    }
    if n == 0 {
        return f64::INFINITY;
    }
    let mut s: Vec<f64> = (0..n * n)
        .map(|k| {
            let (i, j) = (k / n, k % n);
            0.5 * g[i * n + j] + 0.5 * g[j * n + i]
        })
        .collect();
    let scale = s.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    if scale == 0.0 {
        return 0.0;
    }
    for v in &mut s {
        *v /= scale;
    }
    let frob = s.iter().map(|v| v * v).sum::<f64>().sqrt();
    let off = |s: &[f64]| -> f64 {
        let mut sum = 0.0;
        for i in 0..n {
            for j in 0..n {
                if i != j {
                    sum += s[i * n + j] * s[i * n + j];
                }
            }
        }
        sum.sqrt()
    };
    for _ in 0..100 {
        if off(&s) <= 1e-15 * frob {
            break;
        }
        for p in 0..n {
            for q in p + 1..n {
                let apq = s[p * n + q];
                if apq == 0.0 {
                    continue;
                }
                let tau = (s[q * n + q] - s[p * n + p]) / (2.0 * apq);
                let t = if tau >= 0.0 {
                    1.0 / (tau + (1.0 + tau * tau).sqrt())
                } else {
                    -1.0 / (-tau + (1.0 + tau * tau).sqrt())
                };
                let c = 1.0 / (1.0 + t * t).sqrt();
                let sn = t * c;
                for k in 0..n {
                    let (kp, kq) = (s[k * n + p], s[k * n + q]);
                    s[k * n + p] = c * kp - sn * kq;
                    s[k * n + q] = sn * kp + c * kq;
                }
                for k in 0..n {
                    let (pk, qk) = (s[p * n + k], s[q * n + k]);
                    s[p * n + k] = c * pk - sn * qk;
                    s[q * n + k] = sn * pk + c * qk;
                }
                s[p * n + q] = 0.0;
                s[q * n + p] = 0.0;
            }
        }
    }
    scale * (0..n).map(|i| s[i * n + i]).fold(f64::INFINITY, f64::min)
}

/// Whether the real conductance `g` (`n × n`, row-major) draws power under
/// every voltage: every entry is finite and the smallest eigenvalue of its
/// symmetric part is at or above `-1e-12 · max |G_ij|`.
pub fn is_passive(g: &[f64], n: usize) -> bool {
    if g.iter().any(|v| !v.is_finite()) {
        return false;
    }
    let scale = g.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
    smallest_symmetric_eigenvalue(g, n) >= -1e-12 * scale
}

#[cfg(test)]
mod tests {
    use super::{
        factor, inverse, is_passive, lu_solve, singular_below, smallest_symmetric_eigenvalue,
    };
    use crate::support::cmatrix::CMatrix;
    use num_complex::Complex64;

    /// The `n × n` matrix of the row-major `rows`.
    fn matrix(rows: &[Complex64], n: usize) -> CMatrix {
        let mut m = CMatrix::new(n);
        for i in 0..n {
            for j in 0..n {
                m.set(i, j, rows[i * n + j]);
            }
        }
        m
    }

    fn re(v: &[f64]) -> Vec<Complex64> {
        v.iter().map(|&x| Complex64::new(x, 0.0)).collect()
    }

    /// `a · b`, written out.
    fn product(a: &CMatrix, b: &CMatrix) -> CMatrix {
        let n = a.order();
        let mut m = CMatrix::new(n);
        for i in 0..n {
            for j in 0..n {
                let mut s = Complex64::ZERO;
                for k in 0..n {
                    s += a.get(i, k) * b.get(k, j);
                }
                m.set(i, j, s);
            }
        }
        m
    }

    /// The pivot sizes of the steps `factor` completed, and the pivot it
    /// stopped at if any.
    fn pivots(a: &CMatrix) -> (Vec<f64>, Option<f64>) {
        let f = factor(a);
        let n = a.order();
        let done = (0..f.steps).map(|k| f.lu[k * n + k].norm()).collect();
        let stop = (f.steps < n).then(|| f.lu[f.steps * n + f.steps].norm());
        (done, stop)
    }

    /// The symmetric part of a matrix decides passivity: `x · G x` reads only
    /// `(G + Gᵀ) / 2`. Each smallest eigenvalue against its closed form (a 2 x 2
    /// symmetric `[[a, b], [b, a]]` has `a ± b`), and passivity on both sides
    /// of the band `-1e-12 · max |G_ij|`.
    #[test]
    fn passivity_reads_the_symmetric_part() {
        // H = I - 11ᵀ / 2 is symmetric and orthogonal, so `H diag(-3, 8, -2, -4) H`
        // (quarter integers, exact in f64) has those eigenvalues on the columns
        // of H, checked below in exact arithmetic.
        let h4: Vec<f64> = [
            -1.0, -11.0, 9.0, 13.0, -11.0, -1.0, -13.0, -9.0, 9.0, -13.0, -1.0, 11.0, 13.0, -9.0,
            11.0, -1.0,
        ]
        .iter()
        .map(|x| x / 4.0)
        .collect();
        for (v, lambda) in [
            ([1.0, -1.0, -1.0, -1.0], -3.0),
            ([-1.0, 1.0, -1.0, -1.0], 8.0),
            ([-1.0, -1.0, 1.0, -1.0], -2.0),
            ([1.0, 1.0, 1.0, -1.0], -4.0),
        ] {
            for i in 0..4 {
                let gv: f64 = (0..4).map(|j| h4[i * 4 + j] * v[j]).sum();
                assert_eq!(gv, lambda * v[i], "row {i} of H diag(-3, 8, -2, -4) H v");
            }
        }
        let dense3 = vec![1.0, 2.0, 3.0, 2.0, 1.0, 2.0, 3.0, 2.0, 1.0];
        for (i, want) in [-2.0, 0.0, 2.0].into_iter().enumerate() {
            let gv = dense3[i * 3] - dense3[i * 3 + 2];
            assert_eq!(gv, want, "row {i} of [[1,2,3],[2,1,2],[3,2,1]] (1, 0, -1)");
        }

        let cases: [(&str, Vec<f64>, usize, f64, bool); 13] = [
            ("[[1,-1],[-1,1]]", vec![1.0, -1.0, -1.0, 1.0], 2, 0.0, true),
            (
                "[[0.1,0.3],[0.3,0.1]]",
                vec![0.1, 0.3, 0.3, 0.1],
                2,
                0.1 - 0.3,
                false,
            ),
            ("[[1,2],[0,1]]", vec![1.0, 2.0, 0.0, 1.0], 2, 0.0, true),
            (
                "[[1,3],[0,1]]",
                vec![1.0, 3.0, 0.0, 1.0],
                2,
                1.0 - 1.5,
                false,
            ),
            (
                "3 x 3 identity",
                vec![1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
                3,
                1.0,
                true,
            ),
            ("3 x 3 zero", vec![0.0; 9], 3, 0.0, true),
            (
                "[[0.1,0.3,0],[0.3,0.1,0],[0,0,0.1]]",
                vec![0.1, 0.3, 0.0, 0.3, 0.1, 0.0, 0.0, 0.0, 0.1],
                3,
                0.1 - 0.3,
                false,
            ),
            // The band: passive down to -1e-12 of the largest entry. A
            // diagonal matrix holds its eigenvalues exactly, so each side of
            // the band reads exactly, at the scale 1 and at the scale 1e3.
            (
                "diag(1, -1e-13)",
                vec![1.0, 0.0, 0.0, -1e-13],
                2,
                -1e-13,
                true,
            ),
            (
                "diag(1, -1e-11)",
                vec![1.0, 0.0, 0.0, -1e-11],
                2,
                -1e-11,
                false,
            ),
            (
                "diag(1e3, -1e-10)",
                vec![1e3, 0.0, 0.0, -1e-10],
                2,
                -1e-10,
                true,
            ),
            (
                "diag(1e3, -1e-8)",
                vec![1e3, 0.0, 0.0, -1e-8],
                2,
                -1e-8,
                false,
            ),
            // Dense matrices that take several sweeps: one or two sweeps leave
            // the smallest eigenvalue of each off by more than 1e-7, and a stop
            // at 1e-3 of the Frobenius norm leaves that of the 4 x 4 off by
            // 1e-5. The 3 x 3 has -2 on (1, 0, -1) and (5 ± √41) / 2 on the
            // vectors (a, b, a).
            ("[[1,2,3],[2,1,2],[3,2,1]]", dense3, 3, -2.0, false),
            ("H diag(-3, 8, -2, -4) H", h4, 4, -4.0, false),
        ];
        for (name, g, n, want, passive) in cases {
            let got = smallest_symmetric_eigenvalue(&g, n);
            assert!(
                (got - want).abs() <= 1e-14,
                "{name}: smallest eigenvalue {got:e}, closed form {want:e}"
            );
            assert_eq!(is_passive(&g, n), passive, "{name}: smallest {got:e}");
        }

        // Scales whose squares overflow or underflow keep the eigenvalue to
        // 1e-14 of the largest entry, so the matrices that are not passive
        // still fail.
        let scaled: [(&str, Vec<f64>, f64, bool); 6] = [
            (
                "[[1e200,1.5e200,0],[1.5e200,1e200,0],[0,0,1e200]]",
                vec![1e200, 1.5e200, 0.0, 1.5e200, 1e200, 0.0, 0.0, 0.0, 1e200],
                1e200 - 1.5e200,
                false,
            ),
            (
                "[[1e200,2e200,0],[2e200,1e200,0],[0,0,1e200]]",
                vec![1e200, 2e200, 0.0, 2e200, 1e200, 0.0, 0.0, 0.0, 1e200],
                1e200 - 2e200,
                false,
            ),
            (
                "[[1,1e308,0],[1e308,1,0],[0,0,1]]",
                vec![1.0, 1e308, 0.0, 1e308, 1.0, 0.0, 0.0, 0.0, 1.0],
                1.0 - 1e308,
                false,
            ),
            (
                "1e308 times the 3 x 3 identity",
                vec![1e308, 0.0, 0.0, 0.0, 1e308, 0.0, 0.0, 0.0, 1e308],
                1e308,
                true,
            ),
            (
                "1e-300 times [[1,2,0],[2,1,0],[0,0,1]]",
                vec![1e-300, 2e-300, 0.0, 2e-300, 1e-300, 0.0, 0.0, 0.0, 1e-300],
                1e-300 - 2e-300,
                false,
            ),
            (
                "1e-300 times [[2,1,0],[1,2,0],[0,0,1]]",
                vec![2e-300, 1e-300, 0.0, 1e-300, 2e-300, 0.0, 0.0, 0.0, 1e-300],
                1e-300,
                true,
            ),
        ];
        for (name, g, want, passive) in scaled {
            let largest = g.iter().fold(0.0_f64, |m, v| m.max(v.abs()));
            let got = smallest_symmetric_eigenvalue(&g, 3);
            assert!(
                (got - want).abs() <= 1e-14 * largest,
                "{name}: smallest eigenvalue {got:e}, closed form {want:e}"
            );
            assert_eq!(is_passive(&g, 3), passive, "{name}: smallest {got:e}");
        }

        // An entry that is not a finite number leaves no eigenvalue, and the
        // matrix is not passive.
        for (name, g) in [
            (
                "a NaN pair off the diagonal",
                vec![1.0, f64::NAN, 0.0, f64::NAN, 1.0, 0.0, 0.0, 0.0, 1.0],
            ),
            (
                "an infinite diagonal entry",
                vec![f64::INFINITY, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
            ),
            ("every entry NaN", vec![f64::NAN; 9]),
            (
                "diag(NaN, NaN, NaN)",
                vec![f64::NAN, 0.0, 0.0, 0.0, f64::NAN, 0.0, 0.0, 0.0, f64::NAN],
            ),
            (
                "[[NaN,0,0],[0,0.1,0],[0,0,0.1]]",
                vec![f64::NAN, 0.0, 0.0, 0.0, 0.1, 0.0, 0.0, 0.0, 0.1],
            ),
        ] {
            let got = smallest_symmetric_eigenvalue(&g, 3);
            assert!(got.is_nan(), "{name}: smallest eigenvalue {got:e}");
            assert!(!is_passive(&g, 3), "{name}: passive");
        }
    }

    /// The largest entry size of `a`.
    fn largest(a: &CMatrix) -> f64 {
        let n = a.order();
        (0..n * n)
            .map(|k| a.get(k / n, k % n).norm())
            .fold(0.0, f64::max)
    }

    /// Partial pivoting solves a matrix whose leading diagonal entry is zero
    /// and one that swaps rows again at its second step, and a pivot at or
    /// below `n · EPSILON · max |a_ij|` reads as singular: an exact zero, and
    /// the last pivot of two matrices singular in exact arithmetic that `f64`
    /// leaves at about 1.1e-16 (the phasor matrix and the normal matrix of one
    /// winding row of a 7.2 kV / 240 V unit). The valid normal matrices of a
    /// 7.2 kV and a 115 kV / 240 V unit keep their last pivot at 1/901 and at
    /// 1.9e-11 of their largest entry.
    #[test]
    fn lu_solve_pivots_and_inverts() {
        let (o, i1) = (Complex64::ZERO, Complex64::ONE);
        let j = Complex64::I;
        let a = matrix(&[o, i1, 2.0 * i1, i1, j, o, 2.0 * i1, o, i1 + j], 3);
        // The second swap moves the multiplier the first step stored.
        let a2 = matrix(&re(&[1.0, 1.0, 0.0, 2.0, 1.0, 1.0, 0.0, 3.0, 1.0]), 3);
        assert_eq!(
            factor(&a2).swaps,
            [1, 2, 2],
            "[[1,1,0],[2,1,1],[0,3,1]]: the row each step swaps in"
        );
        for (name, m) in [("A", &a), ("[[1,1,0],[2,1,1],[0,3,1]]", &a2)] {
            let inv = inverse(m).expect("a regular matrix");
            let id = product(m, &inv);
            for r in 0..3 {
                for c in 0..3 {
                    let want = if r == c { i1 } else { o };
                    assert!(
                        (id.get(r, c) - want).norm() <= 1e-14,
                        "{name} · inverse({name}) at ({r}, {c}) = {}",
                        id.get(r, c)
                    );
                }
            }
        }
        let inv = inverse(&a).expect("A is regular");
        let b = matrix(
            &[
                i1,
                2.0 * j,
                o,
                -i1,
                Complex64::new(0.5, -1.5),
                o,
                0.5 * j,
                3.0 * i1,
                o,
            ],
            3,
        );
        let x = lu_solve(&a, &b).expect("A is regular");
        let want = product(&inv, &b);
        for r in 0..3 {
            for c in 0..3 {
                assert!(
                    (x.get(r, c) - want.get(r, c)).norm() <= 1e-14,
                    "lu_solve(A, B) at ({r}, {c}) = {}, inverse(A) · B = {}",
                    x.get(r, c),
                    want.get(r, c)
                );
            }
        }

        let alpha = Complex64::from_polar(1.0, 2.0 * std::f64::consts::PI / 3.0);
        let rho = 7.2 / 0.24;
        let singular: [(&str, CMatrix); 3] = [
            (
                "[[1, 2, 3], [2, 4, 6], [1, 0, 1]]",
                matrix(&re(&[1.0, 2.0, 3.0, 2.0, 4.0, 6.0, 1.0, 0.0, 1.0]), 3),
            ),
            (
                "[[1, a], [conj(a), 1]]",
                matrix(&[i1, alpha, alpha.conj(), i1], 2),
            ),
            (
                "[[rho^2, -rho], [-rho, 1]]",
                matrix(&re(&[rho * rho, -rho, -rho, 1.0]), 2),
            ),
        ];
        for (name, m) in &singular {
            let n = m.order();
            let threshold = singular_below(m);
            assert_eq!(
                threshold,
                n as f64 * f64::EPSILON * largest(m),
                "{name}: the threshold"
            );
            let (done, stop) = pivots(m);
            assert_eq!(
                done.len(),
                n - 1,
                "{name}: the last pivot is the one that stops, pivots {done:?}, stop {stop:?}, \
                 threshold {threshold:e}"
            );
            let last = stop.expect("a stop");
            assert!(
                last <= threshold,
                "{name}: last pivot {last:e} above the threshold {threshold:e}"
            );
            assert!(
                lu_solve(m, &CMatrix::new(n)).is_none() && inverse(m).is_none(),
                "{name}: pivots {done:?}, last {last:e}, threshold {threshold:e}"
            );
        }
        let (done, stop) = pivots(&singular[0].1);
        assert_eq!(stop, Some(0.0), "the exact zero pivot, pivots {done:?}");

        // A system with an entry that is not finite has no solution to report,
        // and neither does an elimination that overflows: the multiplier of
        // `[[M, M], [M, M]]` at `M = 1e308` is `M² / M²`, `∞ / ∞`, so its last
        // pivot is not a number, where exact arithmetic gives 0.
        let nan = Complex64::new(f64::NAN, 0.0);
        let inf = Complex64::new(f64::INFINITY, 0.0);
        let identity = matrix(&[i1, o, o, i1], 2);
        let big = 1e308;
        let not_finite: [(&str, CMatrix, CMatrix); 4] = [
            (
                "diag(NaN, 1)",
                matrix(&[nan, o, o, i1], 2),
                identity.clone(),
            ),
            (
                "[[1, inf], [0, 1]]",
                matrix(&[i1, inf, o, i1], 2),
                identity.clone(),
            ),
            (
                "a right-hand side with a NaN entry",
                identity.clone(),
                matrix(&[nan, o, o, i1], 2),
            ),
            (
                "[[M, M], [M, M]] at M = 1e308",
                matrix(&re(&[big, big, big, big]), 2),
                identity.clone(),
            ),
        ];
        for (name, m, rhs) in &not_finite {
            assert!(lu_solve(m, rhs).is_none(), "{name}: a solution");
        }
        assert!(
            inverse(&not_finite[0].1).is_none(),
            "diag(NaN, 1): an inverse"
        );
        let (done, stop) = pivots(&not_finite[3].1);
        assert!(
            done.len() == 1 && stop.is_some_and(f64::is_nan),
            "[[M, M], [M, M]]: pivots {done:?}, stop {stop:?}"
        );

        // The last pivot of `[[ρ² + 1, -ρ], [-ρ, 1]]` is `1 / (ρ² + 1)`, so a
        // threshold above 1.9e-11 of the largest entry would call the 115 kV
        // unit singular.
        for (unit, ratio) in [("7.2 kV / 240 V", rho), ("115 kV / 240 V", 115.0 / 0.24)] {
            let valid = matrix(&re(&[ratio * ratio + 1.0, -ratio, -ratio, 1.0]), 2);
            let threshold = singular_below(&valid);
            assert_eq!(
                threshold,
                2.0 * f64::EPSILON * largest(&valid),
                "{unit}: the threshold"
            );
            let (done, stop) = pivots(&valid);
            assert!(
                stop.is_none() && done[1] > threshold,
                "{unit}: pivots {done:?}, stop {stop:?}, threshold {threshold:e}"
            );
            let rhs = matrix(&[i1, o, i1, o], 2);
            let x = lu_solve(&valid, &rhs).expect("the valid matrix is regular");
            // Row 0 sums two terms that nearly cancel (near 27931 for the
            // 7.2 kV unit), so the residual is held relative to their size.
            for r in 0..2 {
                let res = valid.get(r, 0) * x.get(0, 0) + valid.get(r, 1) * x.get(1, 0) - i1;
                let terms = valid.get(r, 0).norm() * x.get(0, 0).norm()
                    + valid.get(r, 1).norm() * x.get(1, 0).norm();
                assert!(
                    res.norm() <= 1e-12 * terms,
                    "{unit}: row {r} of A x - b = {res} against terms of {terms}, x = ({}, {}), \
                     pivots {done:?}",
                    x.get(0, 0),
                    x.get(1, 0)
                );
            }
        }
    }
}
