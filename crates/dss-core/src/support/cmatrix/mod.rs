//! Dense complex matrix, port of `Shared/Ucmatrix.pas` (`TcMatrix`).
//!
//! Storage is column-major like the Pascal original (`Values[(j-1)*Norder + i]`),
//! indices are 0-based. The inversion routine reproduces the exact pivoting
//! algorithm of `TcMatrix.Invert` so that singular/near-singular behavior is
//! bit-for-bit comparable with the reference engine.

#[cfg(test)]
mod tests;

use num_complex::Complex64;
use std::ops::{Index, IndexMut};

/// Complex division bit-faithful to FPC's `ucomplex` `/` operator (Smith's
/// overflow-safe abs-ratio algorithm), which the Pascal `TcMatrix.Invert`
/// cross-term `A[i,j] - A[i,k]*A[k,j]/A[k,k]` uses. `num_complex`'s `/` operator
/// is the naive `(ac+bd)/(c²+d²)` form, which rounds the last bit differently
/// from Smith's — invisible in robust entries but a 1–3 ULP gap in the
/// cancellation-sensitive (resistance) part of an inverted impedance matrix.
/// Pascal `packages/rtl-extra/src/inc/ucomplex.pp` `operator /`. Shared with the
/// Carson DERI `Get_Zint` Bessel ratio `I0(α)/I1(α)`, which uses the same `/`.
#[inline]
pub(crate) fn cdiv_fpc(num: Complex64, den: Complex64) -> Complex64 {
    if den.re.abs() > den.im.abs() {
        let tmp = den.im / den.re;
        let denom = den.re + den.im * tmp;
        Complex64::new(
            (num.re + num.im * tmp) / denom,
            (num.im - num.re * tmp) / denom,
        )
    } else {
        let tmp = den.re / den.im;
        let denom = den.im + den.re * tmp;
        Complex64::new(
            (num.im + num.re * tmp) / denom,
            (-num.re + num.im * tmp) / denom,
        )
    }
}

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
        self.row(n)
            .chain(self.col(n))
            .all(|e| e.re == 0.0 && e.im == 0.0)
    }

    pub fn set(&mut self, row: usize, col: usize, value: Complex64) {
        self[(row, col)] = value;
    }

    pub fn add(&mut self, row: usize, col: usize, value: Complex64) {
        self[(row, col)] += value;
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
        self[(row, col)]
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

    /// Column `j` as a contiguous slice `[(0,j), (1,j), …, (n-1,j)]`; column-major
    /// storage makes a whole column one span.
    #[inline]
    pub fn col(&self, j: usize) -> &[Complex64] {
        debug_assert!(j < self.n);
        &self.values[j * self.n..(j + 1) * self.n]
    }

    /// Mutable column `j` (contiguous, see [`CMatrix::col`]).
    #[inline]
    pub fn col_mut(&mut self, j: usize) -> &mut [Complex64] {
        debug_assert!(j < self.n);
        &mut self.values[j * self.n..(j + 1) * self.n]
    }

    /// Iterator over the columns, each a contiguous slice (Pascal columns).
    pub fn columns(&self) -> impl Iterator<Item = &[Complex64]> {
        // `chunks_exact` rejects a zero stride; an empty matrix has no columns.
        self.values.chunks_exact(self.n.max(1))
    }

    /// Row `i` as a (strided) iterator `[(i,0), (i,1), …, (i,n-1)]`.
    pub fn row(&self, i: usize) -> impl Iterator<Item = &Complex64> {
        debug_assert!(i < self.n);
        self.values[i..].iter().step_by(self.n).take(self.n)
    }

    /// Mutable row `i` (strided, see [`CMatrix::row`]).
    pub fn row_mut(&mut self, i: usize) -> impl Iterator<Item = &mut Complex64> {
        debug_assert!(i < self.n);
        let n = self.n;
        self.values[i..].iter_mut().step_by(n).take(n)
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
        for e in self.row_mut(row) {
            *e = Complex64::ZERO;
        }
    }

    pub fn zero_col(&mut self, col: usize) {
        self.col_mut(col).fill(Complex64::ZERO);
    }

    /// Average of the diagonal elements; zero for an empty matrix.
    pub fn avg_diagonal(&self) -> Complex64 {
        let mut sum = Complex64::ZERO;
        for i in 0..self.n {
            sum += self[(i, i)];
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
                sum += self[(i, j)];
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
                sum += self[(i, j)] * xj;
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

        let mut used = vec![false; l];
        let mut t1 = Complex64::ZERO;
        let mut k = 0usize;

        for _m in 0..l {
            for ll in 0..l {
                if !used[ll] {
                    // Pascal: RMY := Cabs(A[ll,ll]) - Cabs(T1)
                    let rmy = self[(ll, ll)].norm() - t1.norm();
                    if rmy > 0.0 {
                        t1 = self[(ll, ll)];
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
                            // Pascal: A[i,j] - (A[i,k]*A[k,j]) / A[k,k], where `/`
                            // is FPC ucomplex Smith's division (see cdiv_fpc).
                            self[(i, j)] =
                                self[(i, j)] - cdiv_fpc(self[(i, k)] * self[(k, j)], self[(k, k)]);
                        }
                    }
                }
            }

            self[(k, k)] = -self[(k, k)].inv(); // invert and negate the pivot

            for i in 0..l {
                if i != k {
                    self[(i, k)] = self[(i, k)] * self[(k, k)];
                    self[(k, i)] = self[(k, i)] * self[(k, k)];
                }
            }
        }

        self.negate();
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
                // Pascal: get(i,j) - (get(i,elim)*get(elim,j)) / nn, where `/`
                // is FPC ucomplex Smith's division (see cdiv_fpc).
                result.set(
                    ii,
                    jj,
                    self.get(i, j) - cdiv_fpc(self.get(i, elim) * self.get(elim, j), nn),
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
        let mut out = vec![Complex64::ZERO; self.n];
        for j in 0..self.n {
            // Column `j` of `b` is one contiguous span (column-major storage).
            self.mv_mult(&mut out, b.col(j));
            for (i, &v) in out.iter().enumerate() {
                result.set(i, j, v);
            }
        }
        Some(result)
    }
}

/// Element access by `(row, col)`, resolving the column-major offset once
/// (Pascal `A[row, col]`). `matrix[(i, j)]` reads/copies the `Complex64`.
impl Index<(usize, usize)> for CMatrix {
    type Output = Complex64;

    #[inline]
    fn index(&self, (row, col): (usize, usize)) -> &Complex64 {
        &self.values[self.idx(row, col)]
    }
}

impl IndexMut<(usize, usize)> for CMatrix {
    #[inline]
    fn index_mut(&mut self, (row, col): (usize, usize)) -> &mut Complex64 {
        let i = self.idx(row, col);
        &mut self.values[i]
    }
}
