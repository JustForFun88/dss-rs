//! Dense complex matrix, port of `Shared/Ucmatrix.pas` (`TcMatrix`).
//!
//! Storage is column-major like the Pascal original (`Values[(j-1)*Norder + i]`),
//! indices are 0-based. The inversion routine reproduces the exact pivoting
//! algorithm of `TcMatrix.Invert` so that singular/near-singular behavior is
//! bit-for-bit comparable with the reference engine.

use num_complex::Complex64;

/// Error from [`CMatrix::invert`]. Pascal reported this through the
/// `InvertError` field: 1 = allocation failure (impossible here), 2 = singular.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SingularMatrix;

impl std::fmt::Display for SingularMatrix {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "matrix is singular and cannot be inverted")
    }
}

impl std::error::Error for SingularMatrix {}

/// Square dense complex matrix (Pascal `TcMatrix`).
#[derive(Debug, Clone, PartialEq)]
pub struct CMatrix {
    n: usize,
    /// Column-major: element (row i, col j) lives at `values[j * n + i]`.
    values: Vec<Complex64>,
}

impl CMatrix {
    /// `CreateMatrix(N)`: zero-filled `n × n` matrix.
    pub fn new(n: usize) -> Self {
        Self {
            n,
            values: vec![Complex64::ZERO; n * n],
        }
    }

    /// Matrix order (Pascal `Order` property).
    pub fn order(&self) -> usize {
        self.n
    }

    #[inline]
    fn idx(&self, row: usize, col: usize) -> usize {
        debug_assert!(row < self.n && col < self.n);
        col * self.n + row
    }

    /// Zero out the matrix (Pascal `Clear`).
    pub fn clear(&mut self) {
        self.values.fill(Complex64::ZERO);
    }

    /// True when every entry is exactly zero — no epsilon, on purpose.
    pub fn is_zero(&self) -> bool {
        self.values.iter().all(|v| v.re == 0.0 && v.im == 0.0)
    }

    /// True when row `n` and column `n` are both exactly zero
    /// (Pascal `IsColRowZero`, 0-based here).
    pub fn is_col_row_zero(&self, n: usize) -> bool {
        for j in 0..self.n {
            let e = self.values[self.idx(n, j)];
            if e.re != 0.0 || e.im != 0.0 {
                return false;
            }
            let e = self.values[self.idx(j, n)];
            if e.re != 0.0 || e.im != 0.0 {
                return false;
            }
        }
        true
    }

    pub fn set(&mut self, row: usize, col: usize, value: Complex64) {
        let i = self.idx(row, col);
        self.values[i] = value;
    }

    pub fn add(&mut self, row: usize, col: usize, value: Complex64) {
        let i = self.idx(row, col);
        self.values[i] += value;
    }

    /// Set `(row, col)` and mirror to `(col, row)` (Pascal `SetElemsym`).
    pub fn set_sym(&mut self, row: usize, col: usize, value: Complex64) {
        self.set(row, col, value);
        if row != col {
            self.set(col, row, value);
        }
    }

    /// Add at `(row, col)` and mirror to `(col, row)` (Pascal `AddElemsym`).
    pub fn add_sym(&mut self, row: usize, col: usize, value: Complex64) {
        self.add(row, col, value);
        if row != col {
            self.add(col, row, value);
        }
    }

    pub fn get(&self, row: usize, col: usize) -> Complex64 {
        self.values[self.idx(row, col)]
    }

    /// Copy all elements from a same-order matrix; no-op on order mismatch
    /// (Pascal `CopyFrom`).
    pub fn copy_from(&mut self, other: &CMatrix) {
        if self.n == other.n {
            self.values.copy_from_slice(&other.values);
        }
    }

    /// Accumulate all elements from a same-order matrix; no-op on order
    /// mismatch (Pascal `AddFrom`).
    pub fn add_from(&mut self, other: &CMatrix) {
        if self.n == other.n {
            for (v, o) in self.values.iter_mut().zip(&other.values) {
                *v += o;
            }
        }
    }

    /// Raw column-major value slice (Pascal `GetValuesArrayPtr`).
    pub fn values(&self) -> &[Complex64] {
        &self.values
    }

    pub fn values_mut(&mut self) -> &mut [Complex64] {
        &mut self.values
    }

    pub fn zero_row(&mut self, row: usize) {
        for j in 0..self.n {
            let i = self.idx(row, j);
            self.values[i] = Complex64::ZERO;
        }
    }

    pub fn zero_col(&mut self, col: usize) {
        self.values[col * self.n..(col + 1) * self.n].fill(Complex64::ZERO);
    }

    /// Average of the diagonal elements; zero for an empty matrix.
    pub fn avg_diagonal(&self) -> Complex64 {
        let mut sum = Complex64::ZERO;
        for i in 0..self.n {
            sum += self.values[self.idx(i, i)];
        }
        if self.n > 0 { sum / self.n as f64 } else { sum }
    }

    /// Average of the upper-triangle off-diagonal elements.
    pub fn avg_off_diagonal(&self) -> Complex64 {
        let mut sum = Complex64::ZERO;
        let mut ntimes = 0;
        for i in 0..self.n {
            for j in i + 1..self.n {
                ntimes += 1;
                sum += self.values[self.idx(i, j)];
            }
        }
        if ntimes > 0 { sum / ntimes as f64 } else { sum }
    }

    /// `b = A · x` (Pascal `MVmult`; note the Pascal argument order `(b, x)`).
    pub fn mv_mult(&self, b: &mut [Complex64], x: &[Complex64]) {
        debug_assert!(b.len() >= self.n && x.len() >= self.n);
        for (i, bi) in b.iter_mut().enumerate().take(self.n) {
            let mut sum = Complex64::ZERO;
            for (j, &xj) in x.iter().enumerate().take(self.n) {
                sum += self.values[j * self.n + i] * xj;
            }
            *bi = sum;
        }
    }

    pub fn negate(&mut self) {
        for v in &mut self.values {
            *v = -*v;
        }
    }

    /// In-place inversion, the exact algorithm of `TcMatrix.Invert`:
    /// Gauss-Jordan with pivots chosen by largest-magnitude unused diagonal,
    /// no row exchanges.
    ///
    /// TODO(compat): on a singular pivot the matrix is left partially
    /// transformed, exactly like the Pascal code (callers only check the
    /// error). Restore-or-zero on failure once the 1:1 port is complete.
    pub fn invert(&mut self) -> Result<(), SingularMatrix> {
        let l = self.n;
        let a = &mut self.values;
        let idx = |i: usize, j: usize| j * l + i;

        let mut used = vec![false; l];
        let mut t1 = Complex64::ZERO;
        let mut k = 0usize;

        for _m in 0..l {
            for ll in 0..l {
                if !used[ll] {
                    // Pascal: RMY := Cabs(A[ll,ll]) - Cabs(T1)
                    let rmy = a[idx(ll, ll)].norm() - t1.norm();
                    if rmy > 0.0 {
                        t1 = a[idx(ll, ll)];
                        k = ll;
                    }
                }
            }

            // If the best remaining pivot is zero, the matrix is singular.
            if t1.norm() == 0.0 {
                return Err(SingularMatrix);
            }

            t1 = Complex64::ZERO;
            used[k] = true;
            for i in 0..l {
                if i != k {
                    for j in 0..l {
                        if j != k {
                            a[idx(i, j)] =
                                a[idx(i, j)] - a[idx(i, k)] * a[idx(k, j)] / a[idx(k, k)];
                        }
                    }
                }
            }

            a[idx(k, k)] = -a[idx(k, k)].inv(); // invert and negate the pivot

            for i in 0..l {
                if i != k {
                    a[idx(i, k)] = a[idx(i, k)] * a[idx(k, k)];
                    a[idx(k, i)] = a[idx(k, i)] * a[idx(k, k)];
                }
            }
        }

        for v in a.iter_mut() {
            *v = -*v;
        }
        Ok(())
    }

    /// Kron reduction: eliminate row/column `elim` (0-based) and return the
    /// reduced matrix; `None` when the order is 1 or `elim` is out of range
    /// (Pascal returned `NIL`).
    ///
    /// TODO(compat): like the Pascal code, a zero pivot is not checked and
    /// produces non-finite entries; make it an error once the 1:1 port is
    /// complete.
    pub fn kron(&self, elim: usize) -> Option<CMatrix> {
        if self.n <= 1 || elim >= self.n {
            return None;
        }
        let mut result = CMatrix::new(self.n - 1);
        let nn = self.get(elim, elim);
        let mut ii = 0;
        for i in 0..self.n {
            if i == elim {
                continue;
            }
            let mut jj = 0;
            for j in 0..self.n {
                if j == elim {
                    continue;
                }
                result.set(
                    ii,
                    jj,
                    self.get(i, j) - self.get(i, elim) * self.get(elim, j) / nn,
                );
                jj += 1;
            }
            ii += 1;
        }
        Some(result)
    }

    /// `C = A · B` for same-order square matrices; `None` on order mismatch
    /// (Pascal `MtrxMult` returned `NIL`).
    pub fn mtrx_mult(&self, b: &CMatrix) -> Option<CMatrix> {
        if b.n != self.n {
            return None;
        }
        let mut result = CMatrix::new(self.n);
        let mut col = vec![Complex64::ZERO; self.n];
        let mut out = vec![Complex64::ZERO; self.n];
        for j in 0..self.n {
            for (i, c) in col.iter_mut().enumerate() {
                *c = b.get(i, j);
            }
            self.mv_mult(&mut out, &col);
            for (i, &v) in out.iter().enumerate() {
                result.set(i, j, v);
            }
        }
        Some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(re: f64, im: f64) -> Complex64 {
        Complex64::new(re, im)
    }

    fn assert_close(a: Complex64, b: Complex64, tol: f64) {
        assert!((a - b).norm() <= tol, "{a} vs {b}");
    }

    #[test]
    fn set_get_add_and_symmetry() {
        let mut m = CMatrix::new(3);
        m.set(0, 1, c(1.0, 2.0));
        assert_eq!(m.get(0, 1), c(1.0, 2.0));
        assert_eq!(m.get(1, 0), Complex64::ZERO);

        m.add(0, 1, c(1.0, -1.0));
        assert_eq!(m.get(0, 1), c(2.0, 1.0));

        m.set_sym(1, 2, c(5.0, 0.0));
        assert_eq!(m.get(2, 1), c(5.0, 0.0));

        m.add_sym(1, 2, c(1.0, 0.0));
        assert_eq!(m.get(1, 2), c(6.0, 0.0));
        assert_eq!(m.get(2, 1), c(6.0, 0.0));

        // set_sym on the diagonal must not double-apply
        m.set_sym(0, 0, c(9.0, 0.0));
        assert_eq!(m.get(0, 0), c(9.0, 0.0));
    }

    #[test]
    fn is_zero_and_col_row_zero() {
        let mut m = CMatrix::new(2);
        assert!(m.is_zero());
        m.set(1, 0, c(0.0, 1e-300));
        assert!(!m.is_zero()); // exactly-zero test, no epsilon
        assert!(!m.is_col_row_zero(0)); // column 0 has an entry
        assert!(!m.is_col_row_zero(1)); // row 1 has an entry
        m.clear();
        assert!(m.is_col_row_zero(0) && m.is_col_row_zero(1));
    }

    #[test]
    fn mv_mult_matches_hand_computation() {
        // A = [1+j 2; 3 4-j], x = [1; j]
        let mut a = CMatrix::new(2);
        a.set(0, 0, c(1.0, 1.0));
        a.set(0, 1, c(2.0, 0.0));
        a.set(1, 0, c(3.0, 0.0));
        a.set(1, 1, c(4.0, -1.0));
        let x = [c(1.0, 0.0), c(0.0, 1.0)];
        let mut b = [Complex64::ZERO; 2];
        a.mv_mult(&mut b, &x);
        assert_eq!(b[0], c(1.0, 3.0)); // (1+j) + 2j
        assert_eq!(b[1], c(4.0, 4.0)); // 3 + (4-j)j = 3 + 4j + 1 ... = (4, 4)
    }

    #[test]
    fn invert_3x3_against_hand_computed_inverse() {
        // Real test matrix with known inverse:
        // A = [2 0 1; 0 1 0; 1 0 1], A^-1 = [1 0 -1; 0 1 0; -1 0 2]
        let mut a = CMatrix::new(3);
        a.set(0, 0, c(2.0, 0.0));
        a.set(0, 2, c(1.0, 0.0));
        a.set(1, 1, c(1.0, 0.0));
        a.set(2, 0, c(1.0, 0.0));
        a.set(2, 2, c(1.0, 0.0));
        a.invert().unwrap();

        let expected = [[1.0, 0.0, -1.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 2.0]];
        for (i, row) in expected.iter().enumerate() {
            for (j, &e) in row.iter().enumerate() {
                assert_close(a.get(i, j), c(e, 0.0), 1e-12);
            }
        }
    }

    #[test]
    fn invert_complex_roundtrip_is_identity() {
        // Invert then multiply by the original: expect identity to 1e-12.
        let mut a = CMatrix::new(3);
        let entries = [(0.0, (4.0, 1.0)), (1.0, (-1.0, 0.5)), (2.0, (0.3, -0.2))];
        for i in 0..3 {
            for j in 0..3 {
                let (_, (re, im)) = entries[(i + j) % 3];
                let diag_boost = if i == j { 5.0 } else { 0.0 };
                a.set(i, j, c(re + diag_boost, im));
            }
        }
        let original = a.clone();
        a.invert().unwrap();
        let prod = a.mtrx_mult(&original).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                let expect = if i == j { c(1.0, 0.0) } else { Complex64::ZERO };
                assert_close(prod.get(i, j), expect, 1e-12);
            }
        }
    }

    #[test]
    fn invert_singular_reports_error() {
        let mut a = CMatrix::new(2);
        a.set(0, 0, c(1.0, 0.0));
        a.set(0, 1, c(2.0, 0.0));
        a.set(1, 0, c(2.0, 0.0));
        a.set(1, 1, c(4.0, 0.0)); // row2 = 2*row1
        assert_eq!(a.invert(), Err(SingularMatrix));
    }

    #[test]
    fn kron_reduction_2x2_to_1x1() {
        // Y = [y11 y12; y21 y22]; eliminating node 1 gives y11 - y12*y21/y22.
        let mut y = CMatrix::new(2);
        y.set(0, 0, c(10.0, -5.0));
        y.set(0, 1, c(-2.0, 1.0));
        y.set(1, 0, c(-2.0, 1.0));
        y.set(1, 1, c(8.0, -4.0));
        let r = y.kron(1).unwrap();
        assert_eq!(r.order(), 1);
        let expected = c(10.0, -5.0) - c(-2.0, 1.0) * c(-2.0, 1.0) / c(8.0, -4.0);
        assert_eq!(r.get(0, 0), expected);
    }

    #[test]
    fn kron_eliminating_middle_row_keeps_outer_structure() {
        // 3x3 identity with a coupling: eliminating an uncoupled middle node
        // must leave the other diagonal entries untouched.
        let mut y = CMatrix::new(3);
        y.set(0, 0, c(2.0, 0.0));
        y.set(1, 1, c(3.0, 0.0));
        y.set(2, 2, c(4.0, 0.0));
        y.set_sym(0, 2, c(-1.0, 0.0));
        let r = y.kron(1).unwrap();
        assert_eq!(r.order(), 2);
        assert_eq!(r.get(0, 0), c(2.0, 0.0));
        assert_eq!(r.get(1, 1), c(4.0, 0.0));
        assert_eq!(r.get(0, 1), c(-1.0, 0.0));
    }

    #[test]
    fn kron_invalid_inputs_return_none() {
        let m = CMatrix::new(1);
        assert!(m.kron(0).is_none()); // order 1
        let m = CMatrix::new(3);
        assert!(m.kron(3).is_none()); // out of range
    }

    #[test]
    fn mtrx_mult_identity_and_mismatch() {
        let mut a = CMatrix::new(2);
        a.set(0, 0, c(1.0, 2.0));
        a.set(0, 1, c(3.0, -1.0));
        a.set(1, 0, c(0.5, 0.0));
        a.set(1, 1, c(-2.0, 4.0));
        let mut eye = CMatrix::new(2);
        eye.set(0, 0, c(1.0, 0.0));
        eye.set(1, 1, c(1.0, 0.0));
        let prod = a.mtrx_mult(&eye).unwrap();
        assert_eq!(prod, a);

        assert!(a.mtrx_mult(&CMatrix::new(3)).is_none());
    }

    #[test]
    fn averages_zero_rows_and_negate() {
        let mut m = CMatrix::new(2);
        m.set(0, 0, c(2.0, 2.0));
        m.set(1, 1, c(4.0, -2.0));
        m.set(0, 1, c(6.0, 0.0));
        m.set(1, 0, c(100.0, 0.0)); // lower triangle: excluded from off-diag avg
        assert_eq!(m.avg_diagonal(), c(3.0, 0.0));
        assert_eq!(m.avg_off_diagonal(), c(6.0, 0.0));

        m.negate();
        assert_eq!(m.get(0, 0), c(-2.0, -2.0));

        m.zero_row(0);
        assert_eq!(m.get(0, 1), Complex64::ZERO);
        assert_ne!(m.get(1, 0), Complex64::ZERO);
        m.zero_col(0);
        assert_eq!(m.get(1, 0), Complex64::ZERO);
    }

    #[test]
    fn copy_from_and_add_from_respect_order() {
        let mut a = CMatrix::new(2);
        a.set(0, 0, c(1.0, 0.0));
        let mut b = CMatrix::new(2);
        b.copy_from(&a);
        assert_eq!(b.get(0, 0), c(1.0, 0.0));
        b.add_from(&a);
        assert_eq!(b.get(0, 0), c(2.0, 0.0));

        // order mismatch: silently ignored, like the Pascal code
        let mut c3 = CMatrix::new(3);
        c3.copy_from(&a);
        assert!(c3.is_zero());
    }
}
