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

    /// First singular column from the last failed factorization
    /// (KLUSolve `GetSingularCol`).
    pub fn singular_col(&self) -> Option<usize> {
        self.singular_col
    }

    fn assemble(&mut self) -> Result<(), SparseError> {
        if self.matrix.is_none() {
            let m = SparseColMat::try_new_from_triplets(self.n, self.n, &self.triplets)
                .map_err(|e| SparseError::Internal(format!("{e:?}")))?;
            self.matrix = Some(m);
        }
        Ok(())
    }

    /// LU-factor the matrix; no-op if already factored
    /// (KLUSolve `FactorSparseMatrix`). The symbolic analysis is reused
    /// across calls when the sparsity pattern is unchanged.
    pub fn factor(&mut self) -> Result<(), SparseError> {
        if self.factors.is_some() {
            return Ok(());
        }
        self.assemble()?;
        let matrix = self.matrix.as_ref().expect("assembled above");

        if self.symbolic.is_none() {
            let sym = SymbolicLu::try_new(matrix.symbolic())
                .map_err(|e| SparseError::Internal(format!("{e:?}")))?;
            self.symbolic = Some(sym);
        }
        let symbolic = self.symbolic.as_ref().expect("set above").clone();

        match Lu::try_new_with_symbolic(symbolic, matrix.rb()) {
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
        x.copy_from_slice(b);
        let rhs = MatMut::from_column_major_slice_mut(x, self.n, 1);
        self.factors
            .as_ref()
            .expect("factored above")
            .solve_in_place(rhs);
        Ok(())
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
