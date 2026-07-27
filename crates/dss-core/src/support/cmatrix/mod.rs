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

use crate::compat;

/// Which bottom-left cell a two-terminal block stamp writes (see
/// [`CMatrix::stamp_two_terminal_block`]): the symmetric convention `(j+n, i)`
/// or the direct convention `(i+n, j)`. They coincide for a symmetric primitive;
/// the asymmetric sym-components reactor stamp (`SpecType=4`, Z1≠Z2) needs
/// `Direct` (see `Reactor::stamp_series`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StampBl {
    /// bottom-left block at `(j + n, i)`
    Transposed,
    /// bottom-left block at `(i + n, j)`
    Direct,
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

    /// Stamp an `n`×`n` primitive block into the four quadrants of a two-terminal
    /// (order ≥ 2·`n`) YPrim: the two diagonal quadrants get `+value(i,j)`, the
    /// top-right quadrant `-value`, and the bottom-left quadrant `-value` placed
    /// per `bl`. This is the shared `CalcYPrim` stamping loop every 2-terminal PD
    /// element re-derives (`Line`, `Reactor`, `Capacitor`, `Fault`):
    /// `YPrim[i,j] := V; YPrim[i+n,j+n] := V; YPrim[i,j+n] := -V; …`. Each of the
    /// four quadrants is disjoint, so every cell is written exactly once (`set`
    /// overwrites — no accumulation-order dependence).
    pub fn stamp_two_terminal_block<F>(&mut self, n: usize, bl: StampBl, mut value: F)
    where
        F: FnMut(usize, usize) -> Complex64,
    {
        for i in 0..n {
            for j in 0..n {
                let v = value(i, j);
                self.set(i, j, v);
                self.set(i + n, j + n, v);
                self.set(i, j + n, -v);
                match bl {
                    StampBl::Transposed => self.set(j + n, i, -v),
                    StampBl::Direct => self.set(i + n, j, -v),
                }
            }
        }
    }

    /// Stamp a diagonal (no cross-phase coupling) two-terminal primitive: for
    /// each of `count` conductors `i`, the diagonals `(i,i)` and `(i+off,i+off)`
    /// get `+value` and the couplings `(i,i+off)`/`(i+off,i)` get `-value`. `off`
    /// is the inter-terminal conductor offset (usually the element's `nconds`).
    /// Mirrors the wye `CalcYPrim` loop (`Reactor`/`Capacitor`/`Fault`/
    /// `VSConverter`). `value` is a single scalar shared by every conductor.
    pub fn stamp_two_terminal_diag(&mut self, count: usize, off: usize, value: Complex64) {
        let nvalue = -value;
        for i in 0..count {
            self.set(i, i, value);
            self.set(i + off, i + off, value);
            self.set(i, i + off, nvalue);
            self.set(i + off, i, nvalue);
        }
    }

    /// Stamp a single-terminal delta (line-line) primitive by accumulation: for
    /// each phase `i` (1-based `1..=nphases`), couple it to the next conductor `j`
    /// (wrapping to 1 past `nconds`), adding `+value` to the two diagonals and
    /// `-value` to the two couplings. Uses `add` (Pascal `AddElement`), so
    /// overlapping cells accumulate in loop order — replicated exactly to keep the
    /// floating-point sum bit-identical. Mirrors the delta `CalcYPrim` loop
    /// (`Reactor`/`Capacitor`).
    pub fn stamp_delta_series(&mut self, nphases: usize, nconds: usize, value: Complex64) {
        let value2 = -value;
        for i in 1..=nphases {
            let mut j = i + 1;
            if j > nconds {
                j = 1;
            }
            self.add(i - 1, i - 1, value);
            self.add(j - 1, j - 1, value);
            self.add(i - 1, j - 1, value2);
            self.add(j - 1, i - 1, value2);
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

    /// In-place inversion (Pascal `TcMatrix.Invert`), through the Stage F
    /// lane seam: the parity kernel is the exact Pascal algorithm
    /// (Gauss-Jordan with pivots chosen by largest-magnitude unused diagonal,
    /// **no row exchanges**), the default kernel is partial-pivoting
    /// Gauss-Jordan — see [`compat::invert`].
    pub fn invert(&mut self) -> Result<(), SingularMatrix> {
        compat::invert(self)
    }

    /// Kron reduction: eliminate row/column `elim` (0-based) and return the
    /// reduced matrix; `None` when the order is 1 or `elim` is out of range
    /// (Pascal returned `NIL`).
    ///
    /// A zero pivot is **not** guarded: the elimination divides by
    /// `A[elim, elim]`, so an exactly-zero pivot yields IEEE non-finite entries
    /// — defined behavior, identical in both lanes and to upstream, and the same
    /// contract as every other unguarded division in the engine. This is not a
    /// Stage F compat item (nothing inexact is being reproduced), and the guard
    /// an earlier marker asked for cannot be expressed here: `None` already
    /// means "order 1 / index out of range", so a numeric singularity would be
    /// indistinguishable from a shape error. A typed, diagnosable failure
    /// belongs to the P5/miette rung, not to the kernel split.
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
                // is FPC ucomplex Smith's division (see `compat::cdiv`).
                result.set(
                    ii,
                    jj,
                    self.get(i, j) - compat::cdiv(self.get(i, elim) * self.get(elim, j), nn),
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
