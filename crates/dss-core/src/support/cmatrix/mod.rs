//! Dense complex matrix, port of `Shared/Ucmatrix.pas` (`TcMatrix`).
//!
//! Storage is column-major like the Pascal original (`Values[(j-1)*Norder + i]`),
//! indices are 0-based. The inversion routine reproduces the exact pivoting
//! algorithm of `TcMatrix.Invert` so that singular/near-singular behavior is
//! bit-for-bit comparable with the reference engine.

#[cfg(test)]
mod tests;

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

    /// Row-major copy (`out[i*n + j] = (i, j)`), the layout
    /// [`dss_sparse::SparseSet::add_primitive_matrix`] consumes.
    pub fn to_row_major(&self) -> Vec<Complex64> {
        let n = self.n;
        let mut out = Vec::with_capacity(n * n);
        for i in 0..n {
            for j in 0..n {
                out.push(self.values[j * n + i]);
            }
        }
        out
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
