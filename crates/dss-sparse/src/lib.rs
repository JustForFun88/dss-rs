#![forbid(unsafe_code)]
//! Complex sparse LU solver for the DSS engine.
//!
//! Mirrors the KLUSolve call surface the Pascal engine uses
//! (`.inputs/dss_capi/src/Common/KLUSolve.pas`), backed by the pure-Rust
//! [`faer`] solver. Ownership replaces the `NativeUInt` handles of the C API:
//! one [`SparseSet`] value is one KLUSolve "sparse set".
//!
//! `faer::c64` is a type alias for `num_complex::Complex64`, so values flow
//! between the engine and the solver without conversion.

use faer::MatMut;
use faer::linalg::solvers::Solve;
use faer::prelude::Reborrow;
use faer::sparse::linalg::LuError;
use faer::sparse::linalg::solvers::{Lu, SymbolicLu};
use faer::sparse::{SparseColMat, Triplet};
use num_complex::Complex64;

/// Coordinate (COO) triple `(rows, cols, values)` of a sparse matrix's stored
/// nonzeros — the return of [`SparseSet::coo_entries`].
pub type CooEntries = (Vec<usize>, Vec<usize>, Vec<Complex64>);

/// Errors reported by [`SparseSet`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SparseError {
    /// The matrix is structurally or numerically singular.
    /// `col` is the first column where factorization failed
    /// (KLUSolve: `GetSingularCol`).
    Singular { col: usize },
    /// Allocation failure or index overflow inside the solver.
    Internal(String),
    /// `solve` was called with slices whose length differs from the matrix order.
    DimensionMismatch { expected: usize, got: usize },
}

impl std::fmt::Display for SparseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SparseError::Singular { col } => write!(f, "matrix is singular at column {col}"),
            SparseError::Internal(msg) => write!(f, "sparse solver internal error: {msg}"),
            SparseError::DimensionMismatch { expected, got } => {
                write!(
                    f,
                    "dimension mismatch: matrix order {expected}, vector length {got}"
                )
            }
        }
    }
}

impl std::error::Error for SparseError {}

/// A complex sparse matrix with LU factorization, sized at construction.
///
/// Lifecycle mirrors how the DSS engine drives KLUSolve:
/// `NewSparseSet` → [`SparseSet::new`], `ZeroSparseSet` → [`SparseSet::zero`],
/// `AddMatrixElement`/`AddPrimitiveMatrix` → [`SparseSet::add_element`] /
/// [`SparseSet::add_primitive_matrix`], `FactorSparseMatrix` → [`SparseSet::factor`],
/// `SolveSparseSet` → [`SparseSet::solve`].
///
/// The symbolic (pattern) factorization is computed once and reused across
/// numeric refactorizations as long as the pattern is unchanged — the same
/// optimization KLU performs.
pub struct SparseSet {
    n: usize,
    triplets: Vec<Triplet<usize, usize, Complex64>>,
    matrix: Option<SparseColMat<usize, Complex64>>,
    symbolic: Option<SymbolicLu<usize>>,
    factors: Option<Lu<usize, Complex64>>,
    singular_col: Option<usize>,
    /// Per-row equilibration factors applied before factorization
    /// (`row_scale[i] = 1/max_j |A_ij|`). KLU scales by default (`scale=2`,
    /// row-max); without it faer's LU loses ~`log10(max_entry/min_entry)`
    /// digits on badly-scaled circuit matrices — e.g. an ideal switch's
    /// ~1e4 S admittance alongside ~1e-2 S lines drives the IEEE13 Y to
    /// cond ≈ 2e8, and the un-equilibrated factorization is only
    /// backward-stable to ~1e-10, which lets the fixed point limit-cycle
    /// instead of converging cleanly. The factored matrix is
    /// `diag(row_scale)·A`, so solves scale the right-hand side to match
    /// (`x` is unchanged).
    row_scale: Vec<f64>,
}

impl SparseSet {
    /// Create an empty `n × n` sparse set (KLUSolve `NewSparseSet`).
    pub fn new(n: usize) -> Self {
        Self {
            n,
            triplets: Vec::new(),
            matrix: None,
            symbolic: None,
            factors: None,
            singular_col: None,
            row_scale: Vec::new(),
        }
    }

    /// Matrix order (KLUSolve `GetSize`).
    pub fn size(&self) -> usize {
        self.n
    }

    /// Discard all entries and factorizations, keeping the order
    /// (KLUSolve `ZeroSparseSet`).
    pub fn zero(&mut self) {
        self.triplets.clear();
        self.matrix = None;
        self.symbolic = None;
        self.factors = None;
        self.singular_col = None;
    }

    /// Accumulate `value` at `(row, col)`, 0-based. Duplicate entries are
    /// summed when the matrix is assembled (KLUSolve `AddMatrixElement`).
    pub fn add_element(&mut self, row: usize, col: usize, value: Complex64) {
        debug_assert!(row < self.n && col < self.n);
        self.triplets.push(Triplet::new(row, col, value));
        // Pattern may have changed; existing factorizations are stale.
        self.matrix = None;
        self.factors = None;
    }

    /// Stamp a dense primitive admittance matrix (row-major, `nodes.len()`
    /// square) into the system using 1-based node numbers where node 0 is
    /// ground; ground rows/columns are skipped (KLUSolve `AddPrimitiveMatrix`).
    pub fn add_primitive_matrix(&mut self, nodes: &[usize], yprim: &[Complex64]) {
        let order = nodes.len();
        debug_assert_eq!(yprim.len(), order * order);
        for (i, &ni) in nodes.iter().enumerate() {
            if ni == 0 {
                continue;
            }
            for (j, &nj) in nodes.iter().enumerate() {
                if nj == 0 {
                    continue;
                }
                let v = yprim[i * order + j];
                if v != Complex64::ZERO {
                    self.triplets.push(Triplet::new(ni - 1, nj - 1, v));
                }
            }
        }
        self.matrix = None;
        self.factors = None;
    }

    /// Number of nonzero entries in the assembled matrix (KLUSolve `GetNNZ`).
    pub fn nnz(&mut self) -> Result<usize, SparseError> {
        self.assemble()?;
        Ok(self.matrix.as_ref().map_or(0, |m| m.compute_nnz()))
    }

    /// Number of nonzero entries in the factored matrix (KLUSolve `GetSparseNNZ`).
    /// Returns 0 if not yet factored.
    pub fn sparse_nnz(&self) -> usize {
        if let (Some(_), Some(m)) = (&self.factors, &self.matrix) {
            m.compute_nnz()
        } else {
            0
        }
    }

    /// First singular column from the last failed factorization
    /// (KLUSolve `GetSingularCol`).
    pub fn singular_col(&self) -> Option<usize> {
        self.singular_col
    }

    /// Get a single element from the assembled matrix (KLUSolve `GetMatrixElement`).
    pub fn get_element(&mut self, row: usize, col: usize) -> Result<Complex64, SparseError> {
        self.assemble()?;
        let m = self.matrix.as_ref().expect("assembled above");
        let row_idx = m.row_idx_of_col_raw(col);
        let vals = m.val_of_col(col);
        for (k, &r) in row_idx.iter().enumerate() {
            if r == row {
                return Ok(vals[k]);
            }
        }
        Ok(Complex64::ZERO)
    }

    /// Coordinate dump of the **assembled, unfactored** matrix: returns
    /// `(rows, cols, vals)` of every stored nonzero, in column-major (CSC)
    /// order. This is the matrix as stamped from element YPrims, *before* the
    /// row-equilibration applied for factorization (see [`SparseSet::row_scale`]),
    /// so it is solver-independent and directly comparable to the oracle's
    /// `YMatrix.getYSparse(factor=False)`. Duplicate triplets are already summed
    /// by [`SparseSet::assemble`].
    pub fn coo_entries(&mut self) -> Result<CooEntries, SparseError> {
        self.assemble()?;
        let m = self.matrix.as_ref().expect("assembled above");
        let nnz = m.compute_nnz();
        let mut rows = Vec::with_capacity(nnz);
        let mut cols = Vec::with_capacity(nnz);
        let mut vals = Vec::with_capacity(nnz);
        for col in 0..self.n {
            let row_idx = m.row_idx_of_col_raw(col);
            let col_vals = m.val_of_col(col);
            for (k, &r) in row_idx.iter().enumerate() {
                rows.push(r);
                cols.push(col);
                vals.push(col_vals[k]);
            }
        }
        Ok((rows, cols, vals))
    }

    /// Reciprocal condition number estimate, 0 if singular
    /// (KLUSolve `GetRCond`).
    pub fn rcond(&mut self) -> Result<f64, SparseError> {
        self.factor()?;
        let b = vec![Complex64::new(1.0, 0.0); self.n];
        let nrm_b: f64 = (b.iter().map(|v| v.norm_sqr()).sum::<f64>()).sqrt();
        let mut y = vec![Complex64::ZERO; self.n];
        self.solve_one(&b, &mut y)?;
        let nrm_y: f64 = (y.iter().map(|v| v.norm_sqr()).sum::<f64>()).sqrt();
        if nrm_b == 0.0 {
            Ok(1.0)
        } else if nrm_y == 0.0 {
            Ok(0.0)
        } else {
            Ok(nrm_b / nrm_y)
        }
    }

    /// Solve without factoring (must already be factored). The factored
    /// matrix is `diag(row_scale)·A`, so the right-hand side is scaled to
    /// match before the triangular solves (the solution `x` is unchanged).
    fn solve_one(&self, b: &[Complex64], x: &mut [Complex64]) -> Result<(), SparseError> {
        for i in 0..self.n {
            x[i] = b[i] * self.row_scale[i];
        }
        let rhs = MatMut::from_column_major_slice_mut(x, self.n, 1);
        self.factors
            .as_ref()
            .ok_or_else(|| SparseError::Internal("not factored".into()))?
            .solve_in_place(rhs);
        Ok(())
    }

    /// Find connected components (islands) in the matrix graph via union-find
    /// on the sparsity pattern (KLUSolve `FindIslands`).
    /// Returns a component ID per node (0-based).
    pub fn find_islands(&mut self) -> Result<Vec<usize>, SparseError> {
        self.assemble()?;
        let m = self.matrix.as_ref().expect("assembled above");
        let mut parent: Vec<usize> = (0..self.n).collect();
        let mut rank = vec![0usize; self.n];

        fn find(parent: &mut [usize], x: usize) -> usize {
            if parent[x] != x {
                parent[x] = find(parent, parent[x]);
            }
            parent[x]
        }

        fn union(parent: &mut [usize], rank: &mut [usize], x: usize, y: usize) {
            let rx = find(parent, x);
            let ry = find(parent, y);
            if rx != ry {
                match rank[rx].cmp(&rank[ry]) {
                    std::cmp::Ordering::Less => parent[rx] = ry,
                    std::cmp::Ordering::Greater => parent[ry] = rx,
                    std::cmp::Ordering::Equal => {
                        parent[ry] = rx;
                        rank[rx] += 1;
                    }
                }
            }
        }

        for j in 0..m.ncols() {
            let row_idx = m.row_idx_of_col_raw(j);
            for &r in row_idx {
                let i = r;
                if i < self.n && j < self.n {
                    union(&mut parent, &mut rank, i, j);
                }
            }
        }

        let mut components: Vec<usize> = (0..self.n).collect();
        for (i, c) in components.iter_mut().enumerate() {
            *c = find(&mut parent, i);
        }
        Ok(components)
    }

    fn assemble(&mut self) -> Result<(), SparseError> {
        if self.matrix.is_none() {
            let m = SparseColMat::try_new_from_triplets(self.n, self.n, &self.triplets)
                .map_err(|e| SparseError::Internal(format!("{e:?}")))?;
            self.matrix = Some(m);
        }
        Ok(())
    }

    /// Compute the row-max equilibration factors from the assembled matrix
    /// and build `diag(row_scale)·A` (the matrix actually factored). KLU's
    /// default `scale=2` behavior; see [`SparseSet::row_scale`].
    fn build_scaled(&mut self) -> Result<SparseColMat<usize, Complex64>, SparseError> {
        let matrix = self.matrix.as_ref().expect("assembled");
        let mut row_max = vec![0.0f64; self.n];
        for col in 0..self.n {
            let rows = matrix.row_idx_of_col_raw(col);
            let vals = matrix.val_of_col(col);
            for (k, &r) in rows.iter().enumerate() {
                let a = vals[k].norm();
                if a > row_max[r] {
                    row_max[r] = a;
                }
            }
        }
        self.row_scale = row_max
            .iter()
            .map(|&mx| if mx > 0.0 { 1.0 / mx } else { 1.0 })
            .collect();
        // Scale the assembled values in CSC order (column-major).
        let (symbolic, vals) = matrix.parts();
        let mut sval = vals.to_vec();
        let mut pos = 0usize;
        for col in 0..self.n {
            let rows = matrix.row_idx_of_col_raw(col);
            for &r in rows {
                sval[pos] *= self.row_scale[r];
                pos += 1;
            }
        }
        let owned = symbolic
            .to_owned()
            .map_err(|e| SparseError::Internal(format!("{e:?}")))?;
        Ok(SparseColMat::new(owned, sval))
    }

    /// LU-factor the matrix; no-op if already factored
    /// (KLUSolve `FactorSparseMatrix`). The symbolic analysis is reused
    /// across calls when the sparsity pattern is unchanged. The matrix is
    /// row-equilibrated before factorization (see [`SparseSet::row_scale`]).
    pub fn factor(&mut self) -> Result<(), SparseError> {
        if self.factors.is_some() {
            return Ok(());
        }
        self.assemble()?;
        let scaled = self.build_scaled()?;

        if self.symbolic.is_none() {
            let sym = SymbolicLu::try_new(scaled.symbolic())
                .map_err(|e| SparseError::Internal(format!("{e:?}")))?;
            self.symbolic = Some(sym);
        }
        let symbolic = self.symbolic.as_ref().expect("set above").clone();

        match Lu::try_new_with_symbolic(symbolic, scaled.rb()) {
            Ok(lu) => {
                self.factors = Some(lu);
                self.singular_col = None;
                Ok(())
            }
            Err(LuError::SymbolicSingular { index }) => {
                self.singular_col = Some(index);
                Err(SparseError::Singular { col: index })
            }
            Err(LuError::Generic(e)) => Err(SparseError::Internal(format!("{e:?}"))),
        }
    }

    /// Solve `A·x = b`, factoring first if needed (KLUSolve `SolveSparseSet`).
    pub fn solve(&mut self, b: &[Complex64], x: &mut [Complex64]) -> Result<(), SparseError> {
        if b.len() != self.n || x.len() != self.n {
            return Err(SparseError::DimensionMismatch {
                expected: self.n,
                got: if b.len() != self.n { b.len() } else { x.len() },
            });
        }
        self.factor()?;
        self.solve_one(b, x)
    }
}

#[cfg(test)]
mod tests {
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
}
