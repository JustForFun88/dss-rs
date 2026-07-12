//! Real-valued sparse LU path for the NCIM solver's Jacobian.
//!
//! The complex [`SparseSet`](crate::SparseSet) mirrors KLUSolve's default
//! (complex) matrix format; NCIM's Jacobian is instead a **real** double matrix
//! — the Pascal creates it with `NewSparseSet(N)` +
//! `SetOptions(id, MatrixFormat_DoublePrecisionReal)` and drives it with
//! `SetMatrixElement`/`SolveSparseSet`
//! (`.inputs/dss_capi_with_git/src/Common/NCIMSolutionHelper.pas`,
//! `NCIM_BuildJacobian`/`DoNCIMSolution`). [`RealSparseSet`] is that real path,
//! same faer backing, same `#![forbid(unsafe_code)]`.
//!
//! **`set_element` accumulates.** KLUSolve builds its matrix from a coordinate
//! (triplet) list summed at compression time (CSparse `cs_dupl`), so a repeated
//! `(i, j)` stamp adds. This is confirmed by the vendored EPRI KLUSolve C++
//! (`.inputs/electricdss-code-r4088-trunk/VersionC/klusolve/KLUSolve/Source/`):
//! `SetMatrixElement` → `AddElement` appends a triplet (`bSum` is *ignored* —
//! `KLUSystem.cpp:431-448`), and `GetElement` sums every triplet at a cell
//! (`KLUSystem.cpp:465-470`). The DSS-Extensions KLUSolveX *fork* (which adds the
//! `MatrixFormat_DoublePrecisionReal` real path NCIM uses) is not vendored, but
//! it inherits this CSparse pipeline unchanged. NCIM relies on exactly this:
//! `NCIM_BuildJacobian` stamps
//! each non-swing diagonal 2×2 block from the PDE-only `Y_ii`
//! (`[B, G; G, −B]`), then `NCIM_ApplyCurr` stamps the load/gen injection
//! derivative onto the *same* diagonal cells. For the current-injection Newton
//! Jacobian `J = d(Y·V + g(V))/dV`, the diagonal block must be
//! `Y_ii_block + g'_ii_block` — the two stamps **sum**. (Under replace
//! semantics a PQ node would lose its network coupling on the diagonal and the
//! Jacobian would be wrong.) Duplicate stamps are summed in **insertion order**
//! to match KLUSolve/CSparse bit-for-bit, exactly as the complex path does.

use faer::MatMut;
use faer::linalg::solvers::Solve;
use faer::prelude::Reborrow;
use faer::sparse::linalg::LuError;
use faer::sparse::linalg::solvers::{Lu, SymbolicLu};
use faer::sparse::{SparseColMat, Triplet};

use crate::SparseError;

/// Coordinate (COO) triple `(rows, cols, values)` of a real sparse matrix's
/// stored nonzeros — the return of [`RealSparseSet::coo_entries`].
pub type RealCooEntries = (Vec<usize>, Vec<usize>, Vec<f64>);

/// A real (`f64`) sparse matrix with LU factorization, sized at construction.
///
/// Lifecycle mirrors how NCIM drives its Jacobian sparse set:
/// `NewSparseSet` + `SetOptions(…DoublePrecisionReal)` → [`RealSparseSet::new`],
/// `SetMatrixElement` → [`RealSparseSet::set_element`] (accumulates; see the
/// module note), `SolveSparseSet` → [`RealSparseSet::solve`]. NCIM rebuilds the
/// Jacobian from scratch every iteration (`DeleteSparseSet` + `NewSparseSet` in
/// `NCIM_BuildJacobian`), so there is no incremental-refactor optimization to
/// preserve — each solve assembles and factors afresh.
pub struct RealSparseSet {
    n: usize,
    /// Stamped entries in insertion order. Duplicate `(row, col)` are summed at
    /// [`RealSparseSet::assemble`] time **in this order** (not faer's) to match
    /// KLUSolve/CSparse — see the module note.
    triplets: Vec<(usize, usize, f64)>,
    matrix: Option<SparseColMat<usize, f64>>,
    symbolic: Option<SymbolicLu<usize>>,
    factors: Option<Lu<usize, f64>>,
    singular_col: Option<usize>,
    /// Per-row equilibration factors applied before factorization
    /// (`row_scale[i] = 1/max_j |A_ij|`). KLU scales by default (`scale=2`,
    /// row-max); NCIM never calls `SetOptions` to change the scale, so the
    /// Jacobian is factored row-equilibrated exactly like the complex Y path
    /// (see [`SparseSet::row_scale`](crate::SparseSet)). Solving scales the
    /// right-hand side to match; the solution is unchanged.
    row_scale: Vec<f64>,
}

impl RealSparseSet {
    /// Create an empty `n × n` real sparse set
    /// (KLUSolve `NewSparseSet` + `SetOptions(…DoublePrecisionReal)`).
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

    /// Accumulate `value` at `(row, col)`, 0-based (KLUSolve `SetMatrixElement`
    /// on a real-format set). Duplicate `(row, col)` stamps are summed when the
    /// matrix is assembled — the semantics NCIM's Jacobian relies on (module
    /// note). **A zero value is a no-op**: KLUSolve's `SetMatrixElement` routes
    /// to `AddElement`, which drops the stamp before it reaches the coordinate
    /// list (`if (re == 0.0 && im == 0.0) return;` —
    /// `VersionC/klusolve/KLUSolve/Source/KLUSystem.cpp:442-444`, reached via
    /// `KLUSolve.cpp:202`), so an exact-zero cell never becomes a structural
    /// nonzero. NCIM stamps exact zeros in realistic decks (a pure-R load's
    /// `B`-diagonal, a pure-R/pure-X branch's off-diagonal), so without this
    /// guard `nnz`/`coo_entries`/`Export Jacobian` would over-count vs the
    /// oracle. This mirrors the proven complex path, which skips zeros at stamp
    /// time in `add_primitive_matrix` (`lib.rs`). (Cells whose *nonzero* stamps
    /// happen to sum to 0.0 are not dropped here — KLUSolve's post-`cs_dupl`
    /// `csz_dropzeros` would, but the complex Y path never needed it and the
    /// NCIM diagonal `Y_ii + g'_ii` does not cancel exactly; revisit in Stage 3
    /// if an `Export Jacobian` divergence surfaces.)
    pub fn set_element(&mut self, row: usize, col: usize, value: f64) {
        debug_assert!(row < self.n && col < self.n);
        if value == 0.0 {
            return;
        }
        self.triplets.push((row, col, value));
        // Pattern may have changed; existing factorizations are stale.
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

    /// Get a single assembled element (KLUSolve `GetMatrixElement`) — used by
    /// `Export Jacobian`. Returns `0.0` for a structurally absent cell.
    pub fn get_element(&mut self, row: usize, col: usize) -> Result<f64, SparseError> {
        self.assemble()?;
        let m = self.matrix.as_ref().expect("assembled above");
        let row_idx = m.row_idx_of_col_raw(col);
        let vals = m.val_of_col(col);
        for (k, &r) in row_idx.iter().enumerate() {
            if r == row {
                return Ok(vals[k]);
            }
        }
        Ok(0.0)
    }

    /// Coordinate dump of the **assembled, unfactored** matrix: `(rows, cols,
    /// vals)` of every stored nonzero, column-major. Duplicate triplets are
    /// already summed by [`RealSparseSet::assemble`]. Used by `Export Jacobian`.
    pub fn coo_entries(&mut self) -> Result<RealCooEntries, SparseError> {
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

    fn assemble(&mut self) -> Result<(), SparseError> {
        if self.matrix.is_none() {
            // Sum duplicate (row,col) entries in INSERTION order, matching
            // KLUSolve/CSparse (`cs_dupl`) — see the module note. f64 addition
            // is not associative, so the order sets the last bit.
            let mut pos = std::collections::HashMap::<(usize, usize), usize>::with_capacity(
                self.triplets.len(),
            );
            let mut keys: Vec<(usize, usize)> = Vec::with_capacity(self.triplets.len());
            let mut vals: Vec<f64> = Vec::with_capacity(self.triplets.len());
            for &(r, c, v) in &self.triplets {
                match pos.get(&(r, c)) {
                    Some(&i) => vals[i] += v,
                    None => {
                        pos.insert((r, c), keys.len());
                        keys.push((r, c));
                        vals.push(v);
                    }
                }
            }
            let deduped: Vec<Triplet<usize, usize, f64>> = keys
                .iter()
                .zip(&vals)
                .map(|(&(r, c), &v)| Triplet::new(r, c, v))
                .collect();
            let m = SparseColMat::try_new_from_triplets(self.n, self.n, &deduped)
                .map_err(|e| SparseError::Internal(format!("{e:?}")))?;
            self.matrix = Some(m);
        }
        Ok(())
    }

    /// Compute the row-max equilibration factors from the assembled matrix and
    /// build `diag(row_scale)·A` (the matrix actually factored). KLU's default
    /// `scale=2`; see [`RealSparseSet::row_scale`].
    fn build_scaled(&mut self) -> Result<SparseColMat<usize, f64>, SparseError> {
        let matrix = self.matrix.as_ref().expect("assembled");
        let mut row_max = vec![0.0f64; self.n];
        for col in 0..self.n {
            let rows = matrix.row_idx_of_col_raw(col);
            let vals = matrix.val_of_col(col);
            for (k, &r) in rows.iter().enumerate() {
                let a = vals[k].abs();
                if a > row_max[r] {
                    row_max[r] = a;
                }
            }
        }
        self.row_scale = row_max
            .iter()
            .map(|&mx| if mx > 0.0 { 1.0 / mx } else { 1.0 })
            .collect();
        let (symbolic, vals) = matrix.parts();
        let mut sval = vals.to_vec();
        let mut p = 0usize;
        for col in 0..self.n {
            let rows = matrix.row_idx_of_col_raw(col);
            for &r in rows {
                sval[p] *= self.row_scale[r];
                p += 1;
            }
        }
        let owned = symbolic
            .to_owned()
            .map_err(|e| SparseError::Internal(format!("{e:?}")))?;
        Ok(SparseColMat::new(owned, sval))
    }

    /// LU-factor the matrix; no-op if already factored
    /// (KLUSolve `FactorSparseMatrix`). Row-equilibrated before factorization.
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

    fn solve_one(&self, b: &[f64], x: &mut [f64]) -> Result<(), SparseError> {
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

    /// Solve `A·x = b`, factoring first if needed (KLUSolve `SolveSparseSet` on
    /// a real-format set: the Pascal casts the `array of Double` RHS/solution to
    /// `pComplexArray`, but the real matrix format reads/writes `n` plain
    /// doubles).
    pub fn solve(&mut self, b: &[f64], x: &mut [f64]) -> Result<(), SparseError> {
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

    /// 2×2 real system with a known solution.
    #[test]
    fn solves_small_dense_system() {
        let mut s = RealSparseSet::new(2);
        s.set_element(0, 0, 4.0);
        s.set_element(0, 1, -1.0);
        s.set_element(1, 0, -1.0);
        s.set_element(1, 1, 3.0);

        // A·x = b for x = (1, 2): b = (4·1 + (-1)·2, (-1)·1 + 3·2) = (2, 5).
        let b = [2.0, 5.0];
        let mut x = [0.0; 2];
        s.solve(&b, &mut x).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-12, "got {}", x[0]);
        assert!((x[1] - 2.0).abs() < 1e-12, "got {}", x[1]);
    }

    /// A hand 4×4 current-injection-shaped Jacobian: two 2×2 diagonal blocks
    /// (`[B, G; G, −B]`) plus an off-diagonal coupling block, exactly the
    /// pattern `NCIM_BuildJacobian` stamps. Solve against a synthesized RHS.
    #[test]
    fn solves_two_block_jacobian() {
        // Node 1 self: G=10, B=-4 → [B,G;G,-B] = [-4,10;10,4]
        // Node 2 self: G=8,  B=-3 → [-3,8;8,3]
        // Coupling 1->2 and 2->1: G=-5, B=2 → [2,-5;-5,-2]
        let mut s = RealSparseSet::new(4);
        // block (0,0)
        s.set_element(0, 0, -4.0);
        s.set_element(0, 1, 10.0);
        s.set_element(1, 0, 10.0);
        s.set_element(1, 1, 4.0);
        // block (1,1) at rows/cols 2..3
        s.set_element(2, 2, -3.0);
        s.set_element(2, 3, 8.0);
        s.set_element(3, 2, 8.0);
        s.set_element(3, 3, 3.0);
        // coupling (0,1) block
        s.set_element(0, 2, 2.0);
        s.set_element(0, 3, -5.0);
        s.set_element(1, 2, -5.0);
        s.set_element(1, 3, -2.0);
        // coupling (1,0) block
        s.set_element(2, 0, 2.0);
        s.set_element(2, 1, -5.0);
        s.set_element(3, 0, -5.0);
        s.set_element(3, 1, -2.0);

        // Pick x, form b = A·x by hand, recover x.
        let x_true = [0.5, -1.5, 2.0, 0.25];
        let a = [
            [-4.0, 10.0, 2.0, -5.0],
            [10.0, 4.0, -5.0, -2.0],
            [2.0, -5.0, -3.0, 8.0],
            [-5.0, -2.0, 8.0, 3.0],
        ];
        let mut b = [0.0; 4];
        for i in 0..4 {
            for j in 0..4 {
                b[i] += a[i][j] * x_true[j];
            }
        }
        let mut x = [0.0; 4];
        s.solve(&b, &mut x).unwrap();
        for (got, want) in x.iter().zip(&x_true) {
            assert!((got - want).abs() < 1e-10, "got {got}, want {want}");
        }
    }

    /// Duplicate stamps accumulate (KLUSolveX / NCIM diagonal semantics): the
    /// PDE-Y block plus the injection derivative on one cell sum.
    #[test]
    fn duplicate_stamps_accumulate() {
        let mut s = RealSparseSet::new(1);
        s.set_element(0, 0, 3.0); // "Y diagonal contribution"
        s.set_element(0, 0, 1.0); // "injection derivative"
        let mut x = [0.0];
        s.solve(&[8.0], &mut x).unwrap();
        assert!((x[0] - 2.0).abs() < 1e-14, "got {}", x[0]); // 8 / (3+1)
    }

    /// Duplicate `(row, col)` stamps sum in **insertion order**, like
    /// KLUSolve/CSparse `cs_dupl` — f64 addition is not associative, so the order
    /// sets the last bit. Mirrors the complex path's `duplicates_sum_in_insertion_order`.
    #[test]
    fn duplicates_sum_in_insertion_order() {
        let (a, b, d) = (1.1451220523783028, 3.4336493324686748, 5.564622525570931);
        let mut s = RealSparseSet::new(1);
        s.set_element(0, 0, a);
        s.set_element(0, 0, b);
        s.set_element(0, 0, d);
        let got = s.get_element(0, 0).unwrap();
        assert_eq!(
            got.to_bits(),
            ((a + b) + d).to_bits(),
            "must sum in stamp order"
        );
        assert_ne!(((a + b) + d).to_bits(), ((a + d) + b).to_bits());
    }

    /// A zero-valued stamp is dropped, exactly like KLUSolve `AddElement`
    /// (`if (re == 0.0 && im == 0.0) return;`): it never becomes a structural
    /// nonzero, so `nnz`/`coo_entries`/`get_element` omit it. NCIM stamps exact
    /// zeros (a pure-R load's `B` diagonal, a pure-R branch's `G` off-diagonal),
    /// so this keeps `Export Jacobian`/nnz matching the oracle.
    #[test]
    fn zero_stamp_is_dropped() {
        let mut s = RealSparseSet::new(2);
        s.set_element(0, 0, 4.0);
        s.set_element(0, 1, 0.0); // structural zero — must NOT be stored
        s.set_element(1, 1, 3.0);

        assert_eq!(s.nnz().unwrap(), 2, "zero stamp must not inflate nnz");
        let (rows, cols, vals) = s.coo_entries().unwrap();
        assert_eq!(vals.len(), 2);
        assert!(
            !rows.iter().zip(&cols).any(|(&r, &c)| r == 0 && c == 1),
            "cell (0,1) must be structurally absent, got {rows:?}/{cols:?}"
        );
        assert_eq!(s.get_element(0, 1).unwrap(), 0.0);

        // A later nonzero stamp at the same cell is still recorded — the guard
        // only drops the zero value, it does not blacklist the cell.
        s.set_element(0, 1, 7.0);
        assert_eq!(s.nnz().unwrap(), 3);
        assert_eq!(s.get_element(0, 1).unwrap(), 7.0);
    }

    /// A structurally singular matrix (empty column) reports the column.
    #[test]
    fn singular_matrix_reports_column() {
        let mut s = RealSparseSet::new(2);
        s.set_element(0, 0, 1.0);
        let err = s.factor().unwrap_err();
        assert!(matches!(err, SparseError::Singular { .. }), "got {err:?}");
        assert!(s.singular_col().is_some());
    }

    /// Zero + rebuild + refactor, the per-iteration cycle NCIM performs when it
    /// deletes and re-creates the Jacobian.
    #[test]
    fn zero_and_rebuild() {
        let mut s = RealSparseSet::new(1);
        s.set_element(0, 0, 2.0);
        let mut x = [0.0];
        s.solve(&[2.0], &mut x).unwrap();
        assert!((x[0] - 1.0).abs() < 1e-14);

        s.zero();
        s.set_element(0, 0, 4.0);
        s.solve(&[2.0], &mut x).unwrap();
        assert!((x[0] - 0.5).abs() < 1e-14);
    }

    /// Row equilibration (KLU scale=2) does not change the solution on a
    /// badly-scaled matrix.
    #[test]
    fn badly_scaled_matrix_solves() {
        let mut s = RealSparseSet::new(2);
        s.set_element(0, 0, 1e6);
        s.set_element(0, 1, 1e-3);
        s.set_element(1, 0, 1e-3);
        s.set_element(1, 1, 2.0);
        let x_true = [3.0, -7.0];
        let b = [
            1e6 * x_true[0] + 1e-3 * x_true[1],
            1e-3 * x_true[0] + 2.0 * x_true[1],
        ];
        let mut x = [0.0; 2];
        s.solve(&b, &mut x).unwrap();
        assert!((x[0] - x_true[0]).abs() < 1e-9, "got {}", x[0]);
        assert!((x[1] - x_true[1]).abs() < 1e-9, "got {}", x[1]);
    }

    /// Dimension mismatch on solve is reported, not panicked.
    #[test]
    fn dimension_mismatch_reported() {
        let mut s = RealSparseSet::new(2);
        s.set_element(0, 0, 1.0);
        s.set_element(1, 1, 1.0);
        let mut x = [0.0; 3];
        let err = s.solve(&[1.0, 2.0, 3.0], &mut x).unwrap_err();
        assert!(
            matches!(err, SparseError::DimensionMismatch { .. }),
            "got {err:?}"
        );
    }
}
