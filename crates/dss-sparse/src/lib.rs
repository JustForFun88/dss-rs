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

#[cfg(test)]
mod tests;

pub mod compat;

mod real;
pub use real::RealSparseSet;

use faer::MatMut;
use faer::linalg::solvers::Solve;
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

/// Cached dedup skeleton for [`SparseSet::assemble`], populated on the first
/// (HashMap) assembly of a given stamp pattern and reused across rebuilds whose
/// `(r,c)` stamp sequence is unchanged (only the values move — the tap-change /
/// time-series case). Reusing it skips the per-rebuild `HashMap` dedup *and* the
/// faer builder while staying **bit-identical**: the re-accumulation replays the
/// exact same "assign on the first occurrence of a cell, `+=` after" sequence in
/// stamp order, so every summed cell reproduces the HashMap path bit-for-bit,
/// and the values are scattered verbatim into the existing CSC value buffer.
struct AssembleCache {
    /// Triplet index `k` → its dedup cell index (first-seen order). `len ==`
    /// triplet count at build time; the stamp-sequence signature checked to
    /// validate the cache still applies.
    map: Vec<u32>,
    /// Whether triplet `k` is the **first** occurrence of its cell — an
    /// assignment (`cell_vals[cell] = v`), matching the HashMap path's `push`;
    /// subsequent occurrences `+=`. Preserves the exact bits (incl. `-0.0`) of
    /// the first stamp.
    first: Vec<bool>,
    /// Cell index (first-seen) → its `(row, col)`. `len ==` cell count `== nnz`.
    /// The pattern signature: triplet `k` must still stamp `keys[map[k]]`.
    keys: Vec<(u32, u32)>,
    /// Cell index (first-seen) → its position in the assembled matrix's
    /// column-major (CSC) value buffer. `len == nnz`.
    cell_to_csc: Vec<u32>,
    /// Reused per-cell accumulation scratch (`len == nnz`). Every cell is
    /// assigned on its first occurrence, so no pre-zeroing is needed.
    cell_vals: Vec<Complex64>,
}

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
/// optimization KLU performs. The assembled matrix, its dedup skeleton
/// ([`AssembleCache`]) and the row-equilibrated matrix are likewise retained
/// across [`SparseSet::zero`]+restamp cycles so a value-only rebuild (a
/// tap change, a per-step load `Yeq` update) pays only the numeric refactor.
pub struct SparseSet {
    n: usize,
    /// Stamped entries in insertion order. Duplicate `(row, col)` are summed at
    /// [`SparseSet::assemble`] time **in this order** (not faer's) to match
    /// KLUSolve/CSparse — see the note there.
    triplets: Vec<(usize, usize, Complex64)>,
    /// Assembled, unfactored matrix (the stamped Y). Retained as a skeleton
    /// across `zero()` so a same-pattern rebuild overwrites its value buffer in
    /// place. Stale (does not reflect `triplets`) whenever `needs_assemble`.
    matrix: Option<SparseColMat<usize, Complex64>>,
    /// `matrix` does not reflect the current `triplets` — the assemble sentinel
    /// (replaces the old `matrix.is_none()` check so the skeleton can survive
    /// `zero()` for reuse).
    needs_assemble: bool,
    /// Dedup skeleton for the current stamp pattern (see [`AssembleCache`]).
    cache: Option<AssembleCache>,
    /// The last [`SparseSet::assemble`] reused the cached pattern (only values
    /// moved), so the row-equilibrated `scaled` matrix's skeleton is still valid
    /// and its value buffer can be overwritten in place ([`SparseSet::build_scaled`]).
    pattern_reused: bool,
    /// Row-equilibrated matrix actually factored (`diag(row_scale)·matrix`).
    /// Retained across same-pattern rebuilds; its value buffer is the reused
    /// scale scratch.
    scaled: Option<SparseColMat<usize, Complex64>>,
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
            needs_assemble: true,
            cache: None,
            pattern_reused: false,
            scaled: None,
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

    /// Discard all stamped entries, keeping the order (KLUSolve `ZeroSparseSet`).
    ///
    /// The assembled matrix, its dedup [`AssembleCache`], the LU symbolic
    /// analysis and the row-equilibrated matrix are **retained** as reusable
    /// skeletons: the next assemble validates the new stamp pattern against the
    /// cache and, when it matches (a value-only rebuild), refactors without
    /// re-hashing or re-analyzing. A pattern change invalidates them in
    /// [`SparseSet::assemble`].
    pub fn zero(&mut self) {
        self.triplets.clear();
        self.needs_assemble = true;
        self.factors = None;
        self.singular_col = None;
    }

    /// Accumulate `value` at `(row, col)`, 0-based. Duplicate entries are
    /// summed when the matrix is assembled (KLUSolve `AddMatrixElement`).
    pub fn add_element(&mut self, row: usize, col: usize, value: Complex64) {
        debug_assert!(row < self.n && col < self.n);
        self.triplets.push((row, col, value));
        // Stamps changed: the assembled matrix and any factorization are stale.
        self.needs_assemble = true;
        self.factors = None;
    }

    /// Stamp a dense primitive admittance matrix (**row-major**, `nodes.len()`
    /// square) into the system using 1-based node numbers where node 0 is
    /// ground; ground rows/columns are skipped (KLUSolve `AddPrimitiveMatrix`).
    /// For a column-major `TcMatrix` source use
    /// [`SparseSet::add_primitive_matrix_col_major`] to avoid a transpose copy.
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
                    self.triplets.push((ni - 1, nj - 1, v));
                }
            }
        }
        self.needs_assemble = true;
        self.factors = None;
    }

    /// Stamp a dense primitive admittance matrix stored **column-major**
    /// (element `(i, j)` at `yprim[j*order + i]`, the Pascal `TcMatrix` layout)
    /// without first transposing it to row-major. The traversal is the **exact
    /// same** row-major `(i, j)` order as [`SparseSet::add_primitive_matrix`], so
    /// the triplet insertion order — and therefore the dedup summation order — is
    /// identical: only the read index changes, making this bit-for-bit equivalent
    /// to `add_primitive_matrix(nodes, &cmatrix.to_row_major())` with no per-call
    /// allocation.
    pub fn add_primitive_matrix_col_major(&mut self, nodes: &[usize], yprim: &[Complex64]) {
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
                let v = yprim[j * order + i];
                if v != Complex64::ZERO {
                    self.triplets.push((ni - 1, nj - 1, v));
                }
            }
        }
        self.needs_assemble = true;
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
        for (&r, &v) in row_idx.iter().zip(vals) {
            if r == row {
                return Ok(v);
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
            for (&r, &v) in row_idx.iter().zip(col_vals) {
                rows.push(r);
                cols.push(col);
                vals.push(v);
            }
        }
        Ok((rows, cols, vals))
    }

    /// Reciprocal condition number estimate, 0 if singular
    /// (KLUSolve `GetRCond`).
    ///
    /// Cold path (only `GetRCond`, no per-step caller): the two `n`-vectors are
    /// allocated per call rather than kept as scratch fields — P15 item 5's
    /// "reuse-or-accept" resolved as *accept*, keeping the hot solve state small.
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
        for ((xi, &bi), &s) in x.iter_mut().zip(b).zip(&self.row_scale) {
            *xi = bi * s;
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

        // Iterative two-pass path compression (KLUSolve `FindIslands` uses a
        // loop): recursion would overflow the stack on a degenerate ~8500-node
        // chain where the union tree is a long path before compression.
        fn find(parent: &mut [usize], x: usize) -> usize {
            let mut root = x;
            while parent[root] != root {
                root = parent[root];
            }
            let mut cur = x;
            while parent[cur] != root {
                let next = parent[cur];
                parent[cur] = root;
                cur = next;
            }
            root
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
            for &i in m.row_idx_of_col_raw(j) {
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

    /// Whether the cached dedup skeleton still applies to the current stamp
    /// sequence: same triplet count, and every triplet `k` stamps the same
    /// `(row, col)` its cached cell holds. `O(nnz)` integer compare, no hashing.
    fn cache_pattern_matches(&self) -> bool {
        let Some(cache) = self.cache.as_ref() else {
            return false;
        };
        if cache.map.len() != self.triplets.len() {
            return false;
        }
        for (k, &(r, c, _)) in self.triplets.iter().enumerate() {
            let (kr, kc) = cache.keys[cache.map[k] as usize];
            if kr as usize != r || kc as usize != c {
                return false;
            }
        }
        true
    }

    /// Whether the current order and triplet count fit the `u32` cache indices.
    fn cacheable(&self) -> bool {
        self.n <= u32::MAX as usize && self.triplets.len() <= u32::MAX as usize
    }

    fn assemble(&mut self) -> Result<(), SparseError> {
        if !self.needs_assemble {
            return Ok(());
        }

        // Fast path: the `(r,c)` stamp sequence is unchanged since the cached
        // build (only the values moved — a tap change / per-step `Yeq` update).
        // Re-accumulate each cell in stamp order (assign on the first occurrence,
        // `+=` after — bit-identical to the HashMap path below) and scatter the
        // values into the assembled matrix's existing CSC buffer. No hashing, no
        // faer builder, no reallocation.
        if self.cache_pattern_matches()
            && let (Some(cache), Some(matrix)) = (self.cache.as_mut(), self.matrix.as_mut())
        {
            for (k, &(_, _, v)) in self.triplets.iter().enumerate() {
                let cell = cache.map[k] as usize;
                if cache.first[k] {
                    cache.cell_vals[cell] = v;
                } else {
                    cache.cell_vals[cell] += v;
                }
            }
            let sval = matrix.val_mut();
            for (i, &v) in cache.cell_vals.iter().enumerate() {
                sval[cache.cell_to_csc[i] as usize] = v;
            }
            self.needs_assemble = false;
            self.pattern_reused = true;
            return Ok(());
        }

        // Slow path: sum duplicate (row,col) entries in INSERTION order, matching
        // KLUSolve/CSparse (`cs_dupl`), which accumulates duplicates in the
        // element-stamp order. faer's `try_new_from_triplets` dedups in its
        // own order, which differs in the last ULP on cells fed by several
        // elements (the diagonal/mutual sums). Pre-summing here keeps the
        // assembled system Y bit-identical to the Pascal oracle — whose
        // element YPrims we already match after the FPC-Smith complex-division
        // kernel (`dss-core` `compat::cdiv`) — since the engine stamps elements
        // in creation order, exactly like the reference.
        //
        // The dedup mapping is cached (`AssembleCache`) so subsequent rebuilds of
        // the same pattern take the fast path above.
        let cacheable = self.cacheable();
        let ntriplets = self.triplets.len();
        let mut pos = std::collections::HashMap::<(usize, usize), usize>::with_capacity(ntriplets);
        let mut keys: Vec<(usize, usize)> = Vec::with_capacity(ntriplets);
        let mut vals: Vec<Complex64> = Vec::with_capacity(ntriplets);
        let mut map: Vec<u32> = if cacheable {
            Vec::with_capacity(ntriplets)
        } else {
            Vec::new()
        };
        let mut first: Vec<bool> = if cacheable {
            Vec::with_capacity(ntriplets)
        } else {
            Vec::new()
        };
        for &(r, c, v) in &self.triplets {
            match pos.get(&(r, c)) {
                Some(&i) => {
                    vals[i] += v;
                    if cacheable {
                        map.push(i as u32);
                        first.push(false);
                    }
                }
                None => {
                    let i = keys.len();
                    pos.insert((r, c), i);
                    keys.push((r, c));
                    vals.push(v);
                    if cacheable {
                        map.push(i as u32);
                        first.push(true);
                    }
                }
            }
        }
        let deduped: Vec<Triplet<usize, usize, Complex64>> = keys
            .iter()
            .zip(&vals)
            .map(|(&(r, c), &v)| Triplet::new(r, c, v))
            .collect();
        let m = SparseColMat::try_new_from_triplets(self.n, self.n, &deduped)
            .map_err(|e| SparseError::Internal(format!("{e:?}")))?;

        // Build the cell→CSC-position permutation by walking the assembled
        // matrix column-major and looking each stored (r,c) back up in `pos`.
        self.cache = if cacheable {
            let ncells = keys.len();
            let mut cell_to_csc = vec![0u32; ncells];
            let mut p = 0u32;
            for col in 0..self.n {
                for &r in m.row_idx_of_col_raw(col) {
                    let cell = *pos
                        .get(&(r, col))
                        .expect("assembled cell is in the dedup map");
                    cell_to_csc[cell] = p;
                    p += 1;
                }
            }
            let keys_u32: Vec<(u32, u32)> =
                keys.iter().map(|&(r, c)| (r as u32, c as u32)).collect();
            Some(AssembleCache {
                map,
                first,
                keys: keys_u32,
                cell_to_csc,
                cell_vals: vec![Complex64::ZERO; ncells],
            })
        } else {
            None
        };

        self.matrix = Some(m);
        // The pattern was (re)built: the LU symbolic analysis and the
        // row-equilibrated skeleton no longer apply.
        self.symbolic = None;
        self.scaled = None;
        self.needs_assemble = false;
        self.pattern_reused = false;
        Ok(())
    }

    /// Compute the row-max equilibration factors from the assembled matrix and
    /// build `diag(row_scale)·A` (the matrix actually factored, kept in
    /// `self.scaled`). KLU's default `scale=2` behavior; see
    /// [`SparseSet::row_scale`]. When the assemble reused the cached pattern
    /// (`pattern_reused`) the row-equilibrated skeleton is unchanged, so only its
    /// value buffer is overwritten — no reallocation, no symbolic copy.
    fn build_scaled(&mut self) -> Result<(), SparseError> {
        let n = self.n;
        let matrix = self.matrix.as_ref().expect("assembled");

        // row_max into `row_scale`, then invert in place.
        self.row_scale.clear();
        self.row_scale.resize(n, 0.0);
        for col in 0..n {
            let rows = matrix.row_idx_of_col_raw(col);
            let vals = matrix.val_of_col(col);
            for (&r, v) in rows.iter().zip(vals) {
                let a = v.norm();
                if a > self.row_scale[r] {
                    self.row_scale[r] = a;
                }
            }
        }
        for s in self.row_scale.iter_mut() {
            *s = if *s > 0.0 { 1.0 / *s } else { 1.0 };
        }

        // Reuse the row-equilibrated skeleton in place when the pattern held.
        let can_reuse = self.pattern_reused
            && self
                .scaled
                .as_ref()
                .is_some_and(|s| s.val().len() == matrix.compute_nnz());
        if can_reuse {
            let row_scale = &self.row_scale;
            let scaled = self.scaled.as_mut().expect("checked is_some");
            let (_, sval) = scaled.parts_mut();
            let mut p = 0usize;
            for col in 0..n {
                let rows = matrix.row_idx_of_col_raw(col);
                let vals = matrix.val_of_col(col);
                for (&r, v) in rows.iter().zip(vals) {
                    sval[p] = v * row_scale[r];
                    p += 1;
                }
            }
        } else {
            let mut sval = Vec::with_capacity(matrix.compute_nnz());
            for col in 0..n {
                let rows = matrix.row_idx_of_col_raw(col);
                let vals = matrix.val_of_col(col);
                for (&r, v) in rows.iter().zip(vals) {
                    sval.push(v * self.row_scale[r]);
                }
            }
            let owned = matrix
                .symbolic()
                .to_owned()
                .map_err(|e| SparseError::Internal(format!("{e:?}")))?;
            self.scaled = Some(SparseColMat::new(owned, sval));
        }
        Ok(())
    }

    /// LU-factor the matrix; no-op if already factored
    /// (KLUSolve `FactorSparseMatrix`). The symbolic analysis is reused
    /// across calls when the sparsity pattern is unchanged (including across
    /// `zero()`+restamp). The matrix is row-equilibrated before factorization
    /// (see [`SparseSet::row_scale`]).
    pub fn factor(&mut self) -> Result<(), SparseError> {
        if self.factors.is_some() {
            return Ok(());
        }
        self.assemble()?;
        self.build_scaled()?;
        let scaled = self.scaled.as_ref().expect("built by build_scaled");

        if self.symbolic.is_none() {
            let sym = SymbolicLu::try_new(scaled.symbolic())
                .map_err(|e| SparseError::Internal(format!("{e:?}")))?;
            self.symbolic = Some(sym);
        }
        let symbolic = self.symbolic.as_ref().expect("set above").clone();

        match Lu::try_new_with_symbolic(symbolic, scaled.as_ref()) {
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
