//! Sparse matrix linear-algebra ops in compressed-coordinate (COO) form —
//! a 1:1 port of Pascal `Common/Sparse_Math.pas` (`Tsparse_matrix` +
//! `Tsparse_Complex`, EPRI, Davis Montenegro 2018, after Sudarshan Khasnis's
//! Java `geeksforgeeks` implementation).
//!
//! Both classes store their non-zeros as a flat list of `(row, col, value)`
//! triples in **insertion order**: [`SparseInt::insert`] / [`SparseComplex::insert`]
//! accumulate into an existing `(r, c)` cell (overwriting its value) else append a
//! new triple at the end. That insertion order *is* the storage order, and it is
//! the order every downstream consumer (the `Export IncMatrix`/`Laplacian` CSVs,
//! A-Diakoptics `Contours`/`ZLL`/`ZCC`) reads back — so `add`/`Transpose`/
//! `multiply` must reproduce the Pascal loops exactly, including the counting-sort
//! ordering in `Transpose` and the block-walk in `multiply`.
//!
//! The integer form (`SparseInt`) backs the branch-to-node incidence matrix and
//! its Laplacian (`solution/inc_matrix.rs`); the complex form (`SparseComplex`)
//! backs the A-Diakoptics matrices (Part II). This is a byte-faithful transcription
//! — the ops are not mathematically "cleaned up" (e.g. `multiply` assumes
//! column-sorted row blocks that the incidence matrix does not actually provide;
//! `add`/`multiply` use `re<>0 AND im<>0` drop tests): the goldens pin the exact
//! upstream behavior.
//!
//! Complex values use `num_complex::Complex64` (the engine-wide complex type at
//! this point in the tree). `Sparse_Math`'s complex ops use only `+`, `*`, and
//! conjugation — never `/` or `sqrt` — so they are bit-identical to the FPC
//! `ucomplex` forms and need no `DssComplex64` FPC-division wrapper.

#[cfg(test)]
mod tests;

use num_complex::Complex64;

/// Pascal `Tsparse_matrix`: an integer sparse matrix in COO form. `data` holds
/// one `[row, col, val]` triple per stored non-zero, in insertion order (public,
/// matching Pascal `data: array of array of Integer`, so the exporters can walk
/// it directly).
#[derive(Debug, Clone, Default)]
pub struct SparseInt {
    /// Pascal `row`: the largest row index seen by `insert` (NOT a row count).
    /// Zero-initialized like an FPC object created via `.Create`.
    row: i32,
    /// Pascal `col`: the largest column index seen by `insert`.
    col: i32,
    /// Pascal `len`: the number of stored non-zeros (`= data.len()`).
    len: i32,
    /// Pascal `data`: `[row, col, val]` per non-zero, insertion order.
    pub data: Vec<[i32; 3]>,
}

impl SparseInt {
    /// Pascal `Tsparse_matrix.Create` (the default constructor): all fields zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pascal `sparse_matrix(r, c)`: (re)initialize with declared dimensions and
    /// an empty store.
    pub fn init(&mut self, r: i32, c: i32) {
        self.row = r;
        self.col = c;
        self.len = 0;
        self.data.clear();
    }

    /// Pascal `checkifexists`: index of the `(r, c)` cell, or `-1`. Scans the
    /// whole list and returns the **last** match (Pascal has no early break;
    /// `insert` guarantees at most one).
    fn check_if_exists(&self, r: i32, c: i32) -> i32 {
        let mut result = -1;
        for i in 0..self.len as usize {
            if self.data[i][0] == r && self.data[i][1] == c {
                result = i as i32;
            }
        }
        result
    }

    /// Pascal `insert`: overwrite the value of an existing `(r, c)` cell, else
    /// append a new triple and grow `row`/`col` to cover it.
    pub fn insert(&mut self, r: i32, c: i32, val: i32) -> i32 {
        let lrow = self.check_if_exists(r, c);
        if lrow >= 0 {
            self.data[lrow as usize][2] = val;
        } else {
            self.data.push([r, c, val]);
            self.len += 1;
            if self.col < c {
                self.col = c;
            }
            if self.row < r {
                self.row = r;
            }
        }
        1
    }

    /// Pascal `getrow`: the (cols, vals) of every stored non-zero on row `index`,
    /// in insertion order.
    fn get_row(&self, index: i32) -> (Vec<i32>, Vec<i32>) {
        let mut cols = Vec::new();
        let mut vals = Vec::new();
        for j in 0..self.len as usize {
            if self.data[j][0] == index {
                cols.push(self.data[j][1]);
                vals.push(self.data[j][2]);
            }
        }
        (cols, vals)
    }

    /// Pascal `R_equal`: two rows are "equal" iff they have the same number of
    /// stored columns AND identical column indices (values are ignored — the
    /// `avals`/`bvals` args are unused in the Pascal too).
    fn r_equal(acols: &[i32], bcols: &[i32]) -> bool {
        if acols.len() != bcols.len() {
            return false;
        }
        let mut rlen = 0;
        for idx in 0..acols.len() {
            if acols[idx] - bcols[idx] != 0 {
                rlen += 1;
            }
        }
        rlen == 0
    }

    /// Pascal `Rank` (row-echelon-ish walk, added 2018-08-16 by DM): count rows
    /// that are not equal (by `R_equal`) to any earlier row swept bottom-up.
    pub fn rank(&self) -> i32 {
        let mut result = 0;
        for i in 0..self.row {
            let (acols, _avals) = self.get_row(i);
            if i > 0 {
                let mut j = i - 1;
                let mut flag = true;
                while flag && j >= 0 {
                    let (bcols, _bvals) = self.get_row(j);
                    flag = !Self::r_equal(&acols, &bcols);
                    j -= 1;
                }
                if flag {
                    result += 1;
                }
            } else {
                result += 1;
            }
        }
        result
    }

    /// Pascal `NCols`: the declared column dimension (largest col index seen).
    pub fn ncols(&self) -> i32 {
        self.col
    }

    /// Pascal `NRows`: the declared row dimension (largest row index seen).
    pub fn nrows(&self) -> i32 {
        self.row
    }

    /// Pascal `NZero`: the number of stored non-zeros.
    pub fn nzero(&self) -> i32 {
        self.len
    }

    /// Pascal `reset`: empty the store (dimensions retained).
    pub fn reset(&mut self) {
        self.data.clear();
        self.len = 0;
    }

    /// Pascal `add`: element-wise sum with `b` (same dimensions required). On a
    /// dimension mismatch returns the Pascal sentinel `1×1` matrix holding
    /// `(0,0) = -1`. Reproduces the two upstream tail loops verbatim (they walk
    /// `apos < len-1` / `bpos < b.len-1` and insert `data[apos+1]`'s value — an
    /// off-by-one that the goldens pin).
    pub fn add(&self, b: &SparseInt) -> SparseInt {
        let mut result = SparseInt::new();
        if self.row != b.row || self.col != b.col {
            result.init(1, 1);
            result.insert(0, 0, -1);
            return result;
        }
        result.init(self.row, self.col);
        let mut apos = 0usize;
        let mut bpos = 0usize;
        while apos < self.len as usize && bpos < b.len as usize {
            let (ar, ac) = (self.data[apos][0], self.data[apos][1]);
            let (br, bc) = (b.data[bpos][0], b.data[bpos][1]);
            if ar > br || (ar == br && ac > bc) {
                result.insert(br, bc, b.data[bpos][2]);
                bpos += 1;
            } else if ar < br || (ar == br && ac < bc) {
                result.insert(ar, ac, self.data[apos][2]);
                apos += 1;
            } else {
                let addeval = self.data[apos][2] + b.data[bpos][2];
                if addeval != 0 {
                    result.insert(ar, ac, addeval);
                }
                apos += 1;
                bpos += 1;
            }
        }
        // Inserts the remaining elements (Pascal's `data[apos + 1]` tail quirk).
        while apos < (self.len as usize).saturating_sub(1) {
            result.insert(
                self.data[apos][0],
                self.data[apos][1],
                self.data[apos + 1][2],
            );
            apos += 1;
        }
        while bpos < (b.len as usize).saturating_sub(1) {
            result.insert(b.data[bpos][0], b.data[bpos][1], b.data[bpos + 1][2]);
            bpos += 1;
        }
        result
    }

    /// Pascal `Transpose`: swap rows/cols via a counting sort so the transpose is
    /// stored in ascending row (= original column) order. Reproduces the
    /// pre-population loop (`len` dummy `(i, 0, 0)` inserts to size `Result.data`)
    /// and the stable counting sort exactly.
    pub fn transpose(&self) -> SparseInt {
        let mut result = SparseInt::new();
        result.init(self.col, self.row);
        for i in 1..=self.len {
            result.insert(i, 0, 0);
        }

        let ncol = self.col as usize;
        let mut count = vec![0i32; ncol + 1];
        let mut index = vec![0i32; ncol + 1];
        for i in 0..self.len as usize {
            count[self.data[i][1] as usize] += 1;
        }
        index[0] = 0;
        for i in 1..=ncol {
            index[i] = index[i - 1] + count[i - 1];
        }
        for i in 0..self.len as usize {
            let c = self.data[i][1] as usize;
            let rpos = index[c] as usize;
            index[c] += 1;
            result.data[rpos][0] = self.data[i][1];
            result.data[rpos][1] = self.data[i][0];
            result.data[rpos][2] = self.data[i][2];
        }
        result
    }

    /// Pascal `multiply`: `self * b` (needs `self.col == b.row`; else the `1×1`
    /// `-1` sentinel). Transposes `b`, then for each stored row block of `self`
    /// and each stored row block of `bᵀ` walks the two column lists in lockstep
    /// summing products — a verbatim transcription (it assumes column-sorted row
    /// blocks, which the caller may not provide; the goldens pin the result).
    pub fn multiply(&self, b: &SparseInt) -> SparseInt {
        let mut result = SparseInt::new();
        if self.col != b.row {
            result.init(1, 1);
            result.insert(0, 0, -1);
            return result;
        }
        let b = b.transpose();
        result.init(self.row, b.row);
        let len = self.len as usize;
        let blen = b.len as usize;
        let mut apos = 0usize;
        while apos < len {
            let r = self.data[apos][0];
            let mut bpos = 0usize;
            while bpos < blen {
                let c = b.data[bpos][0];
                let mut tempa = apos;
                let mut tempb = bpos;
                let mut sum = 0i32;
                while tempa < len
                    && self.data[tempa][0] == r
                    && tempb < blen
                    && b.data[tempb][0] == c
                {
                    if self.data[tempa][1] < b.data[tempb][1] {
                        tempa += 1;
                    } else if self.data[tempa][1] > b.data[tempb][1] {
                        tempb += 1;
                    } else {
                        sum += self.data[tempa][2] * b.data[tempb][2];
                        tempa += 1;
                        tempb += 1;
                    }
                }
                if sum != 0 {
                    result.insert(r, c, sum);
                }
                while bpos < blen && b.data[bpos][0] == c {
                    bpos += 1;
                }
            }
            while apos < len && self.data[apos][0] == r {
                apos += 1;
            }
        }
        result
    }
}

/// Pascal `TCmplx_Data`: one complex non-zero.
#[derive(Debug, Clone, Copy)]
pub struct CmplxData {
    pub row: i32,
    pub col: i32,
    pub value: Complex64,
}

/// Pascal `Tsparse_Complex`: a complex sparse matrix in COO form. `cdata` holds
/// the non-zeros in insertion order (public, like the integer form).
#[derive(Debug, Clone, Default)]
pub struct SparseComplex {
    row: i32,
    col: i32,
    len: i32,
    pub cdata: Vec<CmplxData>,
}

impl SparseComplex {
    /// Pascal `Tsparse_Complex.Create` (default constructor): all fields zero.
    pub fn new() -> Self {
        Self::default()
    }

    /// Pascal `sparse_matrix_Cmplx(r, c)`.
    pub fn init(&mut self, r: i32, c: i32) {
        self.row = r;
        self.col = c;
        self.len = 0;
        self.cdata.clear();
    }

    /// Pascal `checkifexists`: last-match index of `(r, c)`, else `-1`.
    fn check_if_exists(&self, r: i32, c: i32) -> i32 {
        let mut result = -1;
        for i in 0..self.len as usize {
            if self.cdata[i].row == r && self.cdata[i].col == c {
                result = i as i32;
            }
        }
        result
    }

    /// Pascal `insert`: overwrite an existing `(r, c)` cell else append.
    pub fn insert(&mut self, r: i32, c: i32, val: Complex64) -> i32 {
        let lrow = self.check_if_exists(r, c);
        if lrow >= 0 {
            self.cdata[lrow as usize].value = val;
        } else {
            self.cdata.push(CmplxData {
                row: r,
                col: c,
                value: val,
            });
            self.len += 1;
            if self.col < c {
                self.col = c;
            }
            if self.row < r {
                self.row = r;
            }
        }
        1
    }

    /// Pascal `getrow`: (cols, vals) of every non-zero on row `index`.
    fn get_row(&self, index: i32) -> (Vec<i32>, Vec<Complex64>) {
        let mut cols = Vec::new();
        let mut vals = Vec::new();
        for j in 0..self.len as usize {
            if self.cdata[j].row == index {
                cols.push(self.cdata[j].col);
                vals.push(self.cdata[j].value);
            }
        }
        (cols, vals)
    }

    /// Pascal `R_equal`: same column count and identical column indices.
    fn r_equal(acols: &[i32], bcols: &[i32]) -> bool {
        if acols.len() != bcols.len() {
            return false;
        }
        let mut rlen = 0;
        for idx in 0..acols.len() {
            if acols[idx] - bcols[idx] != 0 {
                rlen += 1;
            }
        }
        rlen == 0
    }

    /// Pascal `Rank`.
    pub fn rank(&self) -> i32 {
        let mut result = 0;
        for i in 0..self.row {
            let (acols, _avals) = self.get_row(i);
            if i > 0 {
                let mut j = i - 1;
                let mut flag = true;
                while flag && j >= 0 {
                    let (bcols, _bvals) = self.get_row(j);
                    flag = !Self::r_equal(&acols, &bcols);
                    j -= 1;
                }
                if flag {
                    result += 1;
                }
            } else {
                result += 1;
            }
        }
        result
    }

    /// Pascal `NCols`.
    pub fn ncols(&self) -> i32 {
        self.col
    }

    /// Pascal `NRows`.
    pub fn nrows(&self) -> i32 {
        self.row
    }

    /// Pascal `NZero`.
    pub fn nzero(&self) -> i32 {
        self.len
    }

    /// Pascal `reset`.
    pub fn reset(&mut self) {
        self.cdata.clear();
        self.len = 0;
    }

    /// Pascal `add`. NOTE(upstream-quirk): the merge-equal branch drops the sum
    /// only when `re<>0 AND im<>0` (a value with exactly one zero part survives),
    /// and the tail loops carry the same `cdata[apos+1]` off-by-one as the integer
    /// form. Reproduced verbatim.
    pub fn add(&self, b: &SparseComplex) -> SparseComplex {
        let mut result = SparseComplex::new();
        if self.row != b.row || self.col != b.col {
            result.init(1, 1);
            result.insert(0, 0, Complex64::new(-1.0, 0.0));
            return result;
        }
        result.init(self.row, self.col);
        let mut apos = 0usize;
        let mut bpos = 0usize;
        while apos < self.len as usize && bpos < b.len as usize {
            let (ar, ac) = (self.cdata[apos].row, self.cdata[apos].col);
            let (br, bc) = (b.cdata[bpos].row, b.cdata[bpos].col);
            if ar > br || (ar == br && ac > bc) {
                result.insert(br, bc, b.cdata[bpos].value);
                bpos += 1;
            } else if ar < br || (ar == br && ac < bc) {
                result.insert(ar, ac, self.cdata[apos].value);
                apos += 1;
            } else {
                let addeval = self.cdata[apos].value + b.cdata[bpos].value;
                if addeval.re != 0.0 && addeval.im != 0.0 {
                    result.insert(ar, ac, addeval);
                }
                apos += 1;
                bpos += 1;
            }
        }
        while apos < (self.len as usize).saturating_sub(1) {
            result.insert(
                self.cdata[apos].row,
                self.cdata[apos].col,
                self.cdata[apos + 1].value,
            );
            apos += 1;
        }
        while bpos < (b.len as usize).saturating_sub(1) {
            result.insert(
                b.cdata[bpos].row,
                b.cdata[bpos].col,
                b.cdata[bpos + 1].value,
            );
            bpos += 1;
        }
        result
    }

    /// Pascal `Transpose`: counting-sort transpose. The pre-population walks a
    /// `(j, k)` grid of width `self.row` (distinct from the integer form's
    /// `(i, 0)` line, but only used to size `Result.cdata`).
    pub fn transpose(&self) -> SparseComplex {
        let mut result = SparseComplex::new();
        result.init(self.col, self.row);
        let mut j = 0i32;
        let mut k = 0i32;
        for _i in 1..=self.len {
            result.insert(j, k, Complex64::new(0.0, 0.0));
            k += 1;
            if k == self.row {
                j += 1;
                k = 0;
            }
        }
        self.counting_sort_into(&mut result, false);
        result
    }

    /// Pascal `TransposeConj`: transpose with conjugated values. Pre-population is
    /// the `(i, 0)` line form (like the integer transpose).
    pub fn transpose_conj(&self) -> SparseComplex {
        let mut result = SparseComplex::new();
        result.init(self.col, self.row);
        for i in 1..=self.len {
            result.insert(i, 0, Complex64::new(0.0, 0.0));
        }
        self.counting_sort_into(&mut result, true);
        result
    }

    /// The shared counting-sort body of `Transpose`/`TransposeConj`: place each
    /// non-zero at its transposed position, conjugating the value when `conj`.
    fn counting_sort_into(&self, result: &mut SparseComplex, conj: bool) {
        let ncol = self.col as usize;
        let mut count = vec![0i32; ncol + 1];
        let mut index = vec![0i32; ncol + 1];
        for i in 0..self.len as usize {
            count[self.cdata[i].col as usize] += 1;
        }
        index[0] = 0;
        for i in 1..=ncol {
            index[i] = index[i - 1] + count[i - 1];
        }
        for i in 0..self.len as usize {
            let c = self.cdata[i].col as usize;
            let rpos = index[c] as usize;
            index[c] += 1;
            result.cdata[rpos].row = self.cdata[i].col;
            result.cdata[rpos].col = self.cdata[i].row;
            result.cdata[rpos].value = if conj {
                self.cdata[i].value.conj()
            } else {
                self.cdata[i].value
            };
        }
    }

    /// Pascal `multiply`. NOTE(upstream-quirk): the product-drop test is
    /// `re<>0 AND im<>0` (same as `add`), and `b` is transposed (not conj) before
    /// the block walk. Reproduced verbatim.
    pub fn multiply(&self, b: &SparseComplex) -> SparseComplex {
        let mut result = SparseComplex::new();
        if self.col != b.row {
            result.init(1, 1);
            result.insert(0, 0, Complex64::new(-1.0, 0.0));
            return result;
        }
        let b = b.transpose();
        result.init(self.row, b.row);
        let len = self.len as usize;
        let blen = b.len as usize;
        let mut apos = 0usize;
        while apos < len {
            let r = self.cdata[apos].row;
            let mut bpos = 0usize;
            while bpos < blen {
                let c = b.cdata[bpos].row;
                let mut tempa = apos;
                let mut tempb = bpos;
                let mut sum = Complex64::new(0.0, 0.0);
                while tempa < len
                    && self.cdata[tempa].row == r
                    && tempb < blen
                    && b.cdata[tempb].row == c
                {
                    if self.cdata[tempa].col < b.cdata[tempb].col {
                        tempa += 1;
                    } else if self.cdata[tempa].col > b.cdata[tempb].col {
                        tempb += 1;
                    } else {
                        sum += self.cdata[tempa].value * b.cdata[tempb].value;
                        tempa += 1;
                        tempb += 1;
                    }
                }
                if sum.re != 0.0 && sum.im != 0.0 {
                    result.insert(r, c, sum);
                }
                while bpos < blen && b.cdata[bpos].row == c {
                    bpos += 1;
                }
            }
            while apos < len && self.cdata[apos].row == r {
                apos += 1;
            }
        }
        result
    }
}
