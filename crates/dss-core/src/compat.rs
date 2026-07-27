//! Stage F lane split — the `oracle-parity` compat kernels of `dss-core`
//! (`DE_PASCALIZE_PLAN.md` Part IV.2).
//!
//! **This is the only file in `dss-core` allowed to contain the
//! `#[cfg(feature = "oracle-parity")]` string** (enforced by
//! `tests/oracle_parity_cfg_gate.rs`; test attributes and the test harness are
//! the only other sanctioned places workspace-wide).
//!
//! Two build lanes, one engine:
//!
//! - **parity lane** (`--features dss-core/oracle-parity`) — the exact 1:1
//!   engine: every existing oracle gate (byte goldens, checkpoint Y, corpus
//!   floors, iteration counts, discrete states) stays green forever. It never
//!   re-baselines.
//! - **default lane** (no features) — idiomatic Rust kernels; re-baselines once
//!   at Stage F landing (self-goldens only).
//!
//! **Mechanism.** Both implementations of every dual kernel are *always
//! compiled* as plain sibling `*_impl` functions; the cfg selects only which
//! one the short `compat::` alias points at. That is what keeps them
//! unit-testable against each other in *any* build and stops the unselected
//! path from bit-rotting. Call sites are unconditional (`compat::cdiv(a, b)`).
//!
//! **Flip state (F.3 flips one kernel family per commit).** The `Row` column
//! below records which lane each alias resolves to *today*; an unflipped row
//! selects the parity impl in **both** lanes, so it is still bit-neutral and
//! its gates are unchanged in the default build too.
//!
//! **Inventory** (Part IV.2's closed dual-kernel table — do not extend it here;
//! a genuinely new compat item requires editing that table first):
//!
//! | Row | Where | Flipped? |
//! |---|---|---|
//! | complex division | [`cdiv`] — this file | *no split* — measured, see below |
//! | dense inverse (`CMatrix::invert`, `etk_invert`) | [`invert`], [`etk_invert`] — this file | *no split* — measured, see below |
//! | single-point stddev | [`stddev_single_point`] — this file | **yes** (F.3b) |
//! | RPN pi | `dss-parser` `compat::PI` | **yes** (F.3d) |
//! | FPC round | `dss-parser` `compat::round_i32` | **yes** (F.3a) |
//! | solver execution (`Par`, refinement) | `dss-sparse` `compat` | no — declaration only, M3c / WP-R1 own the flip |
//! | Y triplet dedup | *no split* — one shared kernel serves both lanes (IV.1) | — |
//! | sym components | *no split* — measured, see below | — |
//! | Export SeqCurrents `Iresidual` | [`IRESIDUAL_FROM_TERMINAL_1`] — this file | **yes** (F.3c) |
//! | multi-meter `Bus_Int_Duration` | [`BUS_INT_DURATION_WALKS_ALL_BUSES`] — this file | **yes** (F.3c) |
//! | Monitor `BaseFrequency` 60.0 (CLAUDE.md bug 6, deferred here by name) | [`monitor_base_frequency`] — this file | **yes** (F.3c) |
//! | Newton stale `Iterminal` in Powers/Losses (CLAUDE.md bug 5, deferred here as a de-compat decision) | [`POWERS_REUSE_STALE_NEWTON_ITERMINAL`] — this file | **yes** (F.3j) |
//! | report text rendering | F.4 (`F-FMT`) — `compat::fmt` seam | no |
//! | single-site upstream quirks (`PORTING_PLAN` §4.1 rule 4) | the *Single-site upstream quirks* section below | **partly** (F.3k, F.3l…) |
//!
//! Rows 12–13 are not in IV.2's table and do not extend it: they are the two
//! *reproduced* CLAUDE.md upstream bugs whose clean fix that document defers
//! to this pass by name. With them the named-bug set is closed: of the six, two
//! were never reproduced at all (VSConverter's self-aliased `MVMult`, harmonics
//! `Powers`-after-`Currents`) — as is the out-of-range half of
//! `Bus_Int_Duration` — and all four that *are* reproduced now carry the
//! parity/default split (`Iresidual`, the in-range `Bus_Int_Duration`
//! cross-zone overwrite, Monitor `BaseFrequency`, Newton stale `Iterminal`).
//!
//! The last row is likewise not a new *kernel*. IV.2's table enumerates the
//! shared arithmetic kernels — the primitives called from hundreds of sites —
//! and that list is closed. The compat-marker population it does **not**
//! enumerate is governed by `PORTING_PLAN.md` §4.1 rule 4 as updated
//! 2026-07-06, which is the plan text that defers *every* compat marker to this
//! stage and already prescribes its disposition verbatim: "Compat quirks are
//! **not deleted** — each becomes a dual kernel behind `#[cfg(feature =
//! "oracle-parity")]`: the default build gets the correct/precise
//! implementation, the parity build keeps the quirk so every 1:1 oracle gate
//! stays permanently re-runnable." Each such site is argued individually
//! against the Pascal, carries its own expected-value test asserted against
//! [`ORACLE_PARITY`] (so the test is meaningful in *both* lanes), and is listed
//! in the *Single-site upstream quirks* section below.
//!
//! **Dense inverse — why that row resolves to *no split* (F.3f → F.3i,
//! measured 2026-07-26/27).** IV.2's table proposed a partial-pivot (or faer)
//! inverse as the default kernel. The flip was attempted, gated, and then
//! **decomposed to its root**; it is not the ULP-level change the drift model
//! assumes, and no better kernel escapes the finding. Two independent
//! disqualifiers, each measured:
//!
//! 1. **Numerical — one ULP on an ideal switch is worth 14.5× the calibrated
//!    floor, and the *more accurate* kernel is the one that busts it.** Under
//!    the flip, `Test/AutoTrans/Auto1bus-step1.dss` reports `Line.low`
//!    conductor-0 power `1.45235132964843072` where the oracle has exactly `0`
//!    (the `large_near_ideal_source` tier allows `1e-1`). Pinning **only**
//!    `Line`'s series-impedance inversion (`pd/line/solve.rs`) back to the
//!    parity kernel — flip still selected everywhere else, transformer `Zb`
//!    included — restores the exact `0`, so the transformer surface is *not*
//!    the cause. On that deck a `switch=yes r1=1e-6` line has `Z = 1e-9·I`
//!    (exact binary value `1.0000000000000000622…e-9`), which the two kernels
//!    invert to `1e9` (parity) vs `999999999.9999999` (candidate):
//!    **exactly 1 ULP apart, and the candidate is the closer of the two** to the
//!    `Decimal`-60 exact reciprocal `999999999.99999993771…` (error 5.69e-8 vs
//!    the parity kernel's 6.23e-8). Perturbing the parity inverse of that one
//!    entry by −1 ULP reproduces the flip's reading to nine digits
//!    (`1.45235132836994740`), which closes the chain: 1 ULP on a 1e9 S switch
//!    admittance → 1.56e-11 V of split across the switch (≈1 ULP of the 92.95 kV
//!    node) → `y·ΔV` = 1.5625e-2 A → `V·ΔI` = 1.4524 kW. The gain **is** the
//!    family: an ideal switch turns one ULP of node voltage into 15 mA, and
//!    `tests/TOLERANCE_NOTES.md` §near-ideal-source calibrated `i_abs = 0.1` by
//!    decomposition *from a bit-identical Y*. So any dense-inverse kernel that
//!    differs from the parity one by even a single ULP on a `r=1e-6` switch
//!    fails this case — being more accurate does not help, and a faer LU would
//!    be measured against the same wall. (Five further cases sat 1.4–1.8× over
//!    their floors under the flip: `Transformer.sub1` currents on
//!    `EPRITestCircuits/ckt7` and `Examples/StoCtrl_Current_PeakShave`, the
//!    `IEEE_519` Y-fingerprint `trace.im`, `4Bus-YYD` on the r4133 channel.)
//! 2. **Semantic — the two kernels disagree about what a *singular* matrix
//!    leaves behind, and three call sites consume exactly that.** The parity
//!    kernel leaves the input partially transformed (Pascal), the candidate
//!    restores it. `solution::fault_study::compute_ysc` deliberately ignores the
//!    `Err` and stores the transformed `Ysc` on the bus, where `compute_isc`
//!    multiplies `VBus` through it — that is how the `Isc` report mirrors the
//!    upstream value; `report::{show,export}::fault_study` invert a `Yfault`
//!    copy the same way, and `support::line_constants` inverts `FYc` "ignoring
//!    singularity like Pascal does". Flipping would silently change those
//!    reports on a degenerate bus — not a ULP-level difference at all.
//!
//! It was also *not* a branch flip: **(F.3h) 495 872 `Zb` inversions across the
//! 520-case gate, zero `Err` from either kernel** (max disagreement 1.01e-15
//! relative), so the Pascal error-117 substitution never fires and the lanes
//! build the same circuits.
//!
//! The row therefore resolves like "complex division" and "Y triplet dedup":
//! **one shared kernel, no `cfg`**. [`invert_partial_pivot_impl`] /
//! [`etk_invert_partial_pivot_impl`] stay compiled and keep the verdict
//! *asserted* rather than narrated — `tests::dense_inverse_kernels_differ_by_
//! one_ulp_on_an_ideal_switch` pins the 1 ULP with literals, and
//! `tests/compat_dense_inverse.rs` pins the consequence at the physical
//! boundary (that deck's switch split stays exactly `0`).
//!
//! What the attempt left behind, and stays: [`invert_partial_pivot_impl`]
//! normalizes its pivot row through [`cdiv`] instead of `num_complex`'s `/`
//! (which, on a real-valued matrix, computes `x·c/c²` rather than `x/c` and
//! drifted the complex kernel 1 ULP away from its real twin —
//! `mathutil::tests::etk_invert_matches_cmatrix_invert_on_real_matrix` caught
//! it).
//!
//! **Complex division — why that row needs no split (F.3e, plan deviation,
//! settled by measurement).** IV.2's table proposed `num_complex`'s `/` as the
//! default kernel, on the assumption that the parity kernel was a Pascal wart
//! kept only for bit-parity. It is not: FPC `ucomplex`'s `/` is **Smith's
//! algorithm**, the standard robust complex division (C99 `_Cdivd`, LAPACK
//! `dladiv`) — legitimate numerics that happen to also be what upstream uses.
//! Measured against the correctly-rounded quotient (60-digit `Decimal`
//! reference, 20 000 operand pairs spanning 1e-6…1e6 in both Smith branches,
//! 2026-07-27):
//!
//! | kernel | mean rel. error | worst rel. error | outside `|den|` ∈ [1e-154, 1e154] |
//! |---|---|---|---|
//! | Smith (`cdiv_fpc_impl`) | **9.42e-17** | **3.82e-16** | still exact |
//! | naive (`cdiv_std_impl`) | 1.05e-16 | 4.26e-16 | `0` or `NaN` — total loss |
//!
//! So flipping this row would make the *product* lane strictly less accurate
//! and strictly less robust, buying nothing: the parity lane already provides
//! bit-parity, and IV.1 keeps "legitimate numerics with no crate equivalent"
//! (complex Bessel, `dss-sparse` row equilibration) in **both** modes for
//! exactly this reason. The row therefore resolves like "Y triplet dedup":
//! **one shared kernel**. `cdiv_std_impl` stays compiled and its inferiority
//! stays *asserted* (`tests::cdiv_shared_kernel_is_the_more_accurate_one`
//! against a `Decimal`-derived reference table, and
//! `tests::naive_division_collapses_where_smith_stays_exact`), so the verdict
//! cannot rot into folklore. `support::line_constants` already recorded the
//! same conclusion in-tree ("Smith's division … stays permanently") before this
//! measurement independently confirmed it.
//!
//! **Sym components — why that row needs no alias.** The pinned oracle selects
//! the *better-precision* matrices by default: `mathutil.pas:548` ends its
//! initialization with `SelectAs2pVersion(False)` (= "ours" =
//! `SymComp::precise`), and the `official` truncated pair is reachable only
//! through upstream's `DSSCompatFlag.BadPrecision` env flag
//! (`CAPI_DSS.pas:315`), which no gating oracle sets. So parity == default ==
//! `SymComp::precise`, and a cfg alias would select the same impl twice. Both
//! variants stay compiled; `support::mathutil`'s tests pin the gap between
//! them (4.50e-10 relative, the truncated `sin 60°`).

use num_complex::Complex64;

use crate::support::cmatrix::{CMatrix, SingularMatrix};

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------------
// Lane marker
// ---------------------------------------------------------------------------

/// Which lane this engine was compiled in: `true` under
/// `--features oracle-parity` (the bit-compat kernels above are selected),
/// `false` in the default product build.
///
/// Not read by the engine — it exists so a *consumer* can tell the lanes apart.
/// Its one use today is the Stage F test harness (`tests/harness/lane.rs`),
/// which asserts that its own lane const equals this one: if the feature ever
/// stopped propagating into the integration-test crate, every lane branch in
/// the suite would silently run the default policy against a parity engine and
/// the parity gate would evaporate. F.5's differential job reads it to label
/// its two builds.
#[cfg(feature = "oracle-parity")]
pub const ORACLE_PARITY: bool = true;

/// See the parity-lane twin above.
#[cfg(not(feature = "oracle-parity"))]
pub const ORACLE_PARITY: bool = false;

// ---------------------------------------------------------------------------
// Complex division (IV.2 row 1)
// ---------------------------------------------------------------------------

/// Complex division bit-faithful to FPC's `ucomplex` `/` operator (Smith's
/// overflow-safe abs-ratio algorithm), which the Pascal `TcMatrix.Invert`
/// cross-term `A[i,j] - A[i,k]*A[k,j]/A[k,k]` uses. `num_complex`'s `/`
/// operator is the naive `(ac+bd)/(c²+d²)` form, which rounds the last bit
/// differently from Smith's — invisible in robust entries but a 1–3 ULP gap in
/// the cancellation-sensitive (resistance) part of an inverted impedance
/// matrix. Pascal `packages/rtl-extra/src/inc/ucomplex.pp` `operator /`.
/// Shared with the Carson DERI `Get_Zint` Bessel ratio `I0(α)/I1(α)`, which
/// uses the same `/`.
#[inline]
pub fn cdiv_fpc_impl(num: Complex64, den: Complex64) -> Complex64 {
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

/// `num_complex`'s `/` operator — the naive `(ac+bd)/(c²+d²)` form. Compiled,
/// tested, and **not selected in either lane**: it is the measured comparison
/// partner that keeps the "no split" verdict of this row asserted rather than
/// narrated (see the module header and `tests::cdiv_shared_kernel_is_the_more_
/// accurate_one`).
#[inline]
pub fn cdiv_std_impl(num: Complex64, den: Complex64) -> Complex64 {
    num / den
}

/// The complex-division kernel — **one shared implementation, no lane split**
/// (see the module header for the measurement that settled it). Deliberately
/// declared without a `cfg`: both lanes divide with Smith's algorithm.
pub use cdiv_fpc_impl as cdiv;

// ---------------------------------------------------------------------------
// Dense inverse (IV.2 row 2)
// ---------------------------------------------------------------------------

/// In-place complex inversion, the exact algorithm of Pascal
/// `TcMatrix.Invert`: Gauss-Jordan with pivots chosen by largest-magnitude
/// *unused diagonal*, **no row exchanges**, cross-terms through FPC's Smith
/// division.
///
/// On a singular pivot the matrix is left **partially transformed**, exactly
/// like the Pascal code — and that is load-bearing, not a wart awaiting a fix:
/// three call sites deliberately discard the `Err` and go on using the
/// transformed matrix, because upstream does.
/// `solution::fault_study::compute_ysc` stores the partially transformed `Ysc`
/// on the bus and `compute_isc` multiplies `VBus` through it (that is how the
/// `Isc` report mirrors the upstream value); `report::{show,export}::
/// fault_study` invert a `Yfault` copy the same way; `support::line_constants`
/// inverts `FYc` "ignoring singularity like Pascal does". Restoring or zeroing
/// the input on failure would silently change those reports on a degenerate
/// bus, so it is permanent shared-kernel semantics in **both** lanes — and one
/// of the two reasons IV.2 row 2 resolves to *no split*
/// ([`invert_partial_pivot_impl`] restores instead; see the module header).
pub fn invert_gj_no_exchange_impl(m: &mut CMatrix) -> Result<(), SingularMatrix> {
    let l = m.order();

    let mut used = vec![false; l];
    let mut t1 = Complex64::ZERO;
    let mut k = 0usize;

    for _m in 0..l {
        for ll in 0..l {
            if !used[ll] {
                // Pascal: RMY := Cabs(A[ll,ll]) - Cabs(T1)
                let rmy = m[(ll, ll)].norm() - t1.norm();
                if rmy > 0.0 {
                    t1 = m[(ll, ll)];
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
                        // is FPC ucomplex Smith's division. The parity kernel
                        // calls the parity division *directly*, never through
                        // the `cdiv` alias — F.3 flips that alias in the
                        // default lane and this impl must not follow it.
                        m[(i, j)] = m[(i, j)] - cdiv_fpc_impl(m[(i, k)] * m[(k, j)], m[(k, k)]);
                    }
                }
            }
        }

        m[(k, k)] = -m[(k, k)].inv(); // invert and negate the pivot

        for i in 0..l {
            if i != k {
                m[(i, k)] = m[(i, k)] * m[(k, k)];
                m[(k, i)] = m[(k, i)] * m[(k, k)];
            }
        }
    }

    m.negate();
    Ok(())
}

/// In-place complex inversion by Gauss-Jordan elimination **with partial
/// pivoting** (row exchanges) on `[A | I]` — the textbook-stable variant.
///
/// Two deliberate differences from [`invert_gj_no_exchange_impl`]: row
/// exchanges make it stable on matrices whose largest-magnitude entry is off
/// the diagonal, and a singular matrix leaves the input **unchanged** instead
/// of partially transformed (the clean fix the parity impl's compat marker
/// promises). On well-conditioned inputs both agree to ≈1 ULP (measured
/// 2.15e-16 relative on a 3×3 impedance matrix; bound pinned in `tests`).
pub fn invert_partial_pivot_impl(m: &mut CMatrix) -> Result<(), SingularMatrix> {
    let n = m.order();

    // Work on a copy of `A` next to an identity that becomes `A⁻¹`, so a
    // singular matrix can be reported without touching the caller's data.
    let mut a = m.clone();
    let mut inv = CMatrix::new(n);
    for i in 0..n {
        inv.set(i, i, Complex64::ONE);
    }

    for col in 0..n {
        // Partial pivot: the largest-magnitude entry of the remaining column.
        let mut pivot = col;
        for r in (col + 1)..n {
            if a[(r, col)].norm() > a[(pivot, col)].norm() {
                pivot = r;
            }
        }
        if a[(pivot, col)].norm() == 0.0 {
            return Err(SingularMatrix);
        }
        if pivot != col {
            for j in 0..n {
                let t = a[(col, j)];
                a[(col, j)] = a[(pivot, j)];
                a[(pivot, j)] = t;
                let t = inv[(col, j)];
                inv[(col, j)] = inv[(pivot, j)];
                inv[(pivot, j)] = t;
            }
        }

        // Normalize the pivot row. Divides through [`cdiv`] (Smith's), not
        // `num_complex`'s `/`: F.3e measured Smith as the more accurate kernel,
        // and on a real-valued matrix (`im == 0`) it reduces *exactly* to the
        // real division, which is what keeps this kernel bit-consistent with
        // [`etk_invert_partial_pivot_impl`] (pinned by
        // `mathutil::tests::etk_invert_matches_cmatrix_invert_on_real_matrix`).
        let d = a[(col, col)];
        for j in 0..n {
            a[(col, j)] = cdiv(a[(col, j)], d);
            inv[(col, j)] = cdiv(inv[(col, j)], d);
        }

        // Eliminate the column from every other row.
        for r in 0..n {
            if r == col {
                continue;
            }
            let f = a[(r, col)];
            if f == Complex64::ZERO {
                continue;
            }
            for j in 0..n {
                a[(r, j)] = a[(r, j)] - f * a[(col, j)];
                inv[(r, j)] = inv[(r, j)] - f * inv[(col, j)];
            }
        }
    }

    *m = inv;
    Ok(())
}

/// The complex dense-inverse kernel — **one shared implementation, no lane
/// split** (F.3i; see the module header for the two measurements that settled
/// it). Deliberately declared without a `cfg`: both lanes invert with the
/// Pascal no-row-exchange Gauss-Jordan.
pub use invert_gj_no_exchange_impl as invert;

/// In-place inversion of a **real** square matrix in column-major order, the
/// same no-row-exchange Gauss-Jordan as [`invert_gj_no_exchange_impl`] (Pascal
/// `ETKInvert`). Pascal error code 2 (singular) maps to `Err`.
pub fn etk_invert_gj_no_exchange_impl(a: &mut [f64], norder: usize) -> Result<(), SingularMatrix> {
    let l = norder;
    debug_assert_eq!(a.len(), l * l);
    let idx = |i: usize, j: usize| j * l + i;

    let mut used = vec![false; l];
    let mut t1 = 0.0f64;
    let mut k = 0usize;

    for _m in 0..l {
        for ll in 0..l {
            if !used[ll] {
                let rmy = a[idx(ll, ll)].abs() - t1.abs();
                if rmy > 0.0 {
                    t1 = a[idx(ll, ll)];
                    k = ll;
                }
            }
        }

        if t1.abs() == 0.0 {
            return Err(SingularMatrix);
        }

        t1 = 0.0;
        used[k] = true;
        for i in 0..l {
            if i != k {
                for j in 0..l {
                    if j != k {
                        a[idx(i, j)] -= a[idx(i, k)] * a[idx(k, j)] / a[idx(k, k)];
                    }
                }
            }
        }

        a[idx(k, k)] = -1.0 / a[idx(k, k)];

        for i in 0..l {
            if i != k {
                a[idx(i, k)] *= a[idx(k, k)];
                a[idx(k, i)] *= a[idx(k, k)];
            }
        }
    }

    for v in a.iter_mut() {
        *v = -*v;
    }
    Ok(())
}

/// Real counterpart of [`invert_partial_pivot_impl`]: Gauss-Jordan on
/// `[A | I]` with row exchanges, column-major, singular input left unchanged.
pub fn etk_invert_partial_pivot_impl(a: &mut [f64], norder: usize) -> Result<(), SingularMatrix> {
    let n = norder;
    debug_assert_eq!(a.len(), n * n);
    let idx = |i: usize, j: usize| j * n + i;

    let mut work = a.to_vec();
    let mut inv = vec![0.0f64; n * n];
    for i in 0..n {
        inv[idx(i, i)] = 1.0;
    }

    for col in 0..n {
        let mut pivot = col;
        for r in (col + 1)..n {
            if work[idx(r, col)].abs() > work[idx(pivot, col)].abs() {
                pivot = r;
            }
        }
        if work[idx(pivot, col)] == 0.0 {
            return Err(SingularMatrix);
        }
        if pivot != col {
            for j in 0..n {
                work.swap(idx(col, j), idx(pivot, j));
                inv.swap(idx(col, j), idx(pivot, j));
            }
        }

        let d = work[idx(col, col)];
        for j in 0..n {
            work[idx(col, j)] /= d;
            inv[idx(col, j)] /= d;
        }

        for r in 0..n {
            if r == col {
                continue;
            }
            let f = work[idx(r, col)];
            if f == 0.0 {
                continue;
            }
            for j in 0..n {
                work[idx(r, j)] -= f * work[idx(col, j)];
                inv[idx(r, j)] -= f * inv[idx(col, j)];
            }
        }
    }

    a.copy_from_slice(&inv);
    Ok(())
}

/// The real dense-inverse kernel — the counterpart of [`invert`], and **one
/// shared implementation** for the same measured reasons (F.3i).
pub use etk_invert_gj_no_exchange_impl as etk_invert;

// ---------------------------------------------------------------------------
// Single-point standard deviation (IV.2 row 6)
// ---------------------------------------------------------------------------

/// The "standard deviation" of a one-element sample, upstream-faithful: for a
/// single point the Pascal code returns the point *itself* (not 0), reproduced
/// bug-for-bug at the four [`crate::support::mathutil`] entry points.
#[inline]
pub fn stddev_single_point_value_impl(value: f64) -> f64 {
    value
}

/// The standard deviation of a one-element sample: `0.0` — the mathematically
/// correct answer, and a **deliberate divergence** from the oracle (not a
/// tolerance question), pinned by expected-value tests at the four
/// [`crate::support::mathutil`] entry points and at the observable
/// `LoadShape`/`TShape`/`PriceShape` property.
#[inline]
pub fn stddev_single_point_zero_impl(_value: f64) -> f64 {
    0.0
}

#[cfg(feature = "oracle-parity")]
pub use stddev_single_point_value_impl as stddev_single_point;
// F.3: a one-point sample has no spread; the default lane says so.
#[cfg(not(feature = "oracle-parity"))]
pub use stddev_single_point_zero_impl as stddev_single_point;

// ---------------------------------------------------------------------------
// Export SeqCurrents `Iresidual` (IV.2 row 9)
// ---------------------------------------------------------------------------

/// Whether `Export SeqCurrents` prints **terminal 1's** residual current on
/// every terminal row.
///
/// `true` reproduces the upstream bug: Pascal `CalcAndWriteSeqCurrents` sums
/// `cBuffer^[i]` for `i = 1..Ncond` inside the per-terminal loop, missing the
/// `(j-1)*Ncond` offset, so every row repeats terminal 1's residual
/// (`ExportResults.pas`; oracle-proven on IEEE13 `Line.671680`, whose true
/// terminal-2 residual is 9.8e-12 A while the export prints terminal 1's
/// 2.83e-5 A). Deterministic and defined, so the parity lane keeps it.
///
/// `false` sums the row's **own** terminal — the clean fix. The value is what
/// the element's `Iterminal` already holds; only the slice changes, so this
/// is a reporting fix with no effect on any solved quantity.
pub const IRESIDUAL_FROM_TERMINAL_1_PARITY_IMPL: bool = true;
/// See the parity twin above.
pub const IRESIDUAL_FROM_TERMINAL_1_DEFAULT_IMPL: bool = false;

#[cfg(not(feature = "oracle-parity"))]
pub use IRESIDUAL_FROM_TERMINAL_1_DEFAULT_IMPL as IRESIDUAL_FROM_TERMINAL_1;
#[cfg(feature = "oracle-parity")]
pub use IRESIDUAL_FROM_TERMINAL_1_PARITY_IMPL as IRESIDUAL_FROM_TERMINAL_1;

// ---------------------------------------------------------------------------
// Multi-meter `Bus_Int_Duration` (IV.2 row 10)
// ---------------------------------------------------------------------------

/// Whether `CalcReliabilityIndices`' bus-interruption-duration loop walks
/// **every circuit bus** instead of only this meter's zone.
///
/// `true` reproduces the upstream bug (`EnergyMeter.pas:2521`): with more than
/// one EnergyMeter, a bus whose `BusSectionID` was written by *another* meter's
/// sweep is indexed into **this** meter's `FeederSections`, so the later meter
/// overwrites foreign buses' durations from its own sections. In-range section
/// ids make that a deterministic cross-zone overwrite the parity lane keeps
/// (golden `export_busreliability_multimeter`); out-of-range ids are an OOB
/// heap read, proven nondeterministic and never reproduced in either lane (the
/// `.get()` returns `None`).
///
/// `false` walks only the buses this meter's own zone sweep assigned — the
/// clean fix, which makes each meter's durations independent of meter order.
pub const BUS_INT_DURATION_WALKS_ALL_BUSES_PARITY_IMPL: bool = true;
/// See the parity twin above.
pub const BUS_INT_DURATION_WALKS_ALL_BUSES_DEFAULT_IMPL: bool = false;

#[cfg(not(feature = "oracle-parity"))]
pub use BUS_INT_DURATION_WALKS_ALL_BUSES_DEFAULT_IMPL as BUS_INT_DURATION_WALKS_ALL_BUSES;
#[cfg(feature = "oracle-parity")]
pub use BUS_INT_DURATION_WALKS_ALL_BUSES_PARITY_IMPL as BUS_INT_DURATION_WALKS_ALL_BUSES;

// ---------------------------------------------------------------------------
// Monitor base frequency (CLAUDE.md §Known upstream bugs — deferred to Stage F)
// ---------------------------------------------------------------------------

/// A new Monitor's `BaseFrequency`, upstream-faithful: `TMonitorObj.Create`
/// hard-pins `Basefrequency := 60.0` *after* the inherited constructor
/// (`Monitor.pas:472` == r4133 `:552`), overriding the base-class
/// `BaseFrequency := ActiveCircuit.Fundamental` (`CktElement.pas:233`) that
/// every other element gets. Both gating oracles pin 60.0.
///
/// This is not cosmetic: the value is the `fBase` a mode-4 monitor passes into
/// `FlickerMeter` (`Monitor.pas:1657` → `Pstcalc.pas:594`), where `fBase =
/// 50.0` selects the IEC 61000-4-15 230 V/50 Hz lamp weighting coefficients
/// instead of the 120 V/60 Hz set (`Pstcalc.pas:609-626`) — so in a 50 Hz
/// circuit upstream computes Pst with the wrong lamp curve unless the user
/// writes `basefreq=50` by hand.
#[inline]
pub fn monitor_base_frequency_60hz_impl(_fundamental: f64) -> f64 {
    60.0
}

/// A new Monitor's `BaseFrequency`, the clean fix: inherit the circuit's
/// fundamental like every other element. In a 60 Hz circuit — every committed
/// golden and every gated corpus deck that instantiates a Monitor — this is
/// bit-identical to [`monitor_base_frequency_60hz_impl`]; in a 50 Hz circuit it
/// is the **deliberate divergence** that also gives mode-4 flicker the right
/// lamp curve.
#[inline]
pub fn monitor_base_frequency_inherit_impl(fundamental: f64) -> f64 {
    fundamental
}

#[cfg(feature = "oracle-parity")]
pub use monitor_base_frequency_60hz_impl as monitor_base_frequency;
#[cfg(not(feature = "oracle-parity"))]
pub use monitor_base_frequency_inherit_impl as monitor_base_frequency;

// ---------------------------------------------------------------------------
// Newton stale `Iterminal` in Powers/Losses
// (CLAUDE.md §Known upstream bugs — deferred here as a de-compat DECISION)
// ---------------------------------------------------------------------------

/// Whether reported `Powers`/`Losses` reuse the terminal current the **Newton**
/// solver left cached, instead of recomputing it at the converged voltage.
///
/// `true` reproduces the upstream quirk. `DoNewtonSolution`'s final
/// `SumAllCurrents` stamps `Iterminal` from the *pre-final* voltage guess
/// `NodeV_{n-1}` and marks it solved for the current `SolutionCount`; the
/// `NodeV -= dV` update follows it. `CktElement.Get_Powers`/`Get_Losses` read
/// through the cache-aware `ComputeIterminal` and therefore return that
/// one-step-stale current, while `CktElement.Currents` (`GetCurrents`)
/// recomputes fresh at `NodeV_n` — so after `Set algorithm=Newton` upstream
/// reports `S != V·conj(I)` for the same element in the same read. It is
/// deterministic, defined and not state-poisoning, so the parity lane keeps it,
/// and it cannot be escaped by bumping the oracle: the EPRI channel confirms
/// the quirk in every vendored official rev — v9.8 (r3723), v10.2 (r4088),
/// v11.0 (r4133), all fingerprint 0.478 kVA (checked 2026-07-08). See
/// `investigations/newton_stale_iterminal_bug_report.md`.
///
/// `false` recomputes `Iterminal` at `NodeV_n` for all three reads — the clean
/// fix named at the reproduction site since the port, which restores the
/// identity `S = V·conj(I)` that `Powers` is *defined* by, in every algorithm.
/// The lanes differ **only** after a Newton solve: after every fixed-point /
/// direct / harmonic solve the cache is already invalid at read time, so both
/// lanes recompute the same current and every other deck is bit-identical.
///
/// **What replaces the lost gate signal.** Newton and the normal fixed point
/// converge to the same voltages in the same iteration count on the `newton*`
/// corpus decks, so this staleness was the *only* channel there that proved
/// Newton dispatch was wired at all — the default lane no longer exposes it
/// (`tests/harness/lane.rs::LANE_SKIP_ELEM_POWERS` excludes those decks'
/// powers/losses in that lane only; the parity lane still compares them against
/// both oracles). The replacement is stronger and runs in **both** lanes:
/// `exec::tests::newton` asserts the staleness *inside the engine* — after
/// `algorithm=Newton` the solver-cached `Iterminal` differs from a fresh
/// recompute at the converged `NodeV`, and after the normal algorithm it does
/// not — which is a direct assertion that `DoNewtonSolution` ran, rather than
/// an inference from a reported power.
pub const POWERS_REUSE_STALE_NEWTON_ITERMINAL_PARITY_IMPL: bool = true;
/// See the parity twin above.
pub const POWERS_REUSE_STALE_NEWTON_ITERMINAL_DEFAULT_IMPL: bool = false;

#[cfg(not(feature = "oracle-parity"))]
pub use POWERS_REUSE_STALE_NEWTON_ITERMINAL_DEFAULT_IMPL as POWERS_REUSE_STALE_NEWTON_ITERMINAL;
#[cfg(feature = "oracle-parity")]
pub use POWERS_REUSE_STALE_NEWTON_ITERMINAL_PARITY_IMPL as POWERS_REUSE_STALE_NEWTON_ITERMINAL;

// ---------------------------------------------------------------------------
// Single-site upstream quirks
// (PORTING_PLAN §4.1 rule 4, as updated 2026-07-06 — see the module header)
// ---------------------------------------------------------------------------
//
// Each row below is one reproduction site, not a shared kernel: a deterministic
// upstream mistake whose clean fix its own compat marker had already
// named, argued here against the Pascal and split by lane. Parity keeps the
// quirk (so every existing oracle gate stays re-runnable); the default lane
// gets the fix and an expected-value test pinned against `ORACLE_PARITY`.
//
// Membership rule, so this section cannot become a dumping ground: a site
// qualifies only if (i) upstream's behavior is deterministic and defined (UB is
// never reproduced in *either* lane, per CLAUDE.md), (ii) the clean fix is
// unambiguous — the Pascal itself, a sibling class, or the source's own comment
// says what was meant — and (iii) the divergence is pinned by a test rather
// than merely narrated.

/// Whether an explicit `Bus2=` on an **Isource** fails to latch, so a later
/// `Bus1=` silently overwrites it.
///
/// `true` reproduces the upstream quirk: `TIsourceObj.PropertySideEffects`
/// (`Isource.pas:221`) has no `Bus2` case at all, so `Bus2Defined` never
/// becomes `TRUE` — while `TVsourceObj.PropertySideEffects` (`Vsource.pas:498`)
/// sets it on exactly that property. The `Bus1` side effect then re-derives the
/// grounded-Y default `bus1.0.0…` unconditionally, so on an Isource an explicit
/// `Bus2` survives only if it is parsed *after* `Bus1` in the same edit.
///
/// `false` latches it like the sibling class does — the clean fix named at the
/// site since the port. The lanes differ only when `Bus2=` precedes `Bus1=` on
/// one Isource edit, which no golden and no gated corpus deck does.
pub const ISOURCE_BUS2_NEVER_LATCHES_PARITY_IMPL: bool = true;
/// See the parity twin above.
pub const ISOURCE_BUS2_NEVER_LATCHES_DEFAULT_IMPL: bool = false;

#[cfg(not(feature = "oracle-parity"))]
pub use ISOURCE_BUS2_NEVER_LATCHES_DEFAULT_IMPL as ISOURCE_BUS2_NEVER_LATCHES;
#[cfg(feature = "oracle-parity")]
pub use ISOURCE_BUS2_NEVER_LATCHES_PARITY_IMPL as ISOURCE_BUS2_NEVER_LATCHES;

/// Whether `Like=` on a **CapControl** drops the `ControlSignal` reference.
///
/// `true` reproduces the upstream quirk: `TCapControlObj.MakeLike`
/// (`CapControl.pas:446-490`) copies every other reference and field —
/// `ControlledElement`, `MonitoredElement`, the user model, both snapshots —
/// but never `ctrlSignalShape` or its name, so a clone of a `type=Follow`
/// CapControl is left with no signal to follow and silently controls nothing.
///
/// `false` copies them alongside the other references — the clean fix named at
/// the site. The lanes differ only for `Like=` on a Follow-type CapControl,
/// which no golden and no gated corpus deck contains.
pub const CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL_PARITY_IMPL: bool = true;
/// See the parity twin above.
pub const CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL_DEFAULT_IMPL: bool = false;

#[cfg(not(feature = "oracle-parity"))]
pub use CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL_DEFAULT_IMPL as CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL;
#[cfg(feature = "oracle-parity")]
pub use CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL_PARITY_IMPL as CAPCONTROL_MAKELIKE_DROPS_CONTROL_SIGNAL;

/// Whether `Like=` on a **LineSpacing** drops the equivalent-spacing fields.
///
/// `true` reproduces the upstream quirk: `TLineSpacingObj.MakeLike` copies
/// `NConds`/`NPhases`/`FX`/`FY`/`Units` and stops, leaving `detailed`,
/// `eqDistPhPh`, `eqDistPhN`, `avgPhaseHeight` and `avgNeutralHeight` at their
/// `Create` defaults — even though the base class has already copied the
/// `PrpSequence` that marks them as set. The clone therefore reports a spacing
/// the source object does not have.
///
/// `false` copies them too — the clean fix named at the site. Those five fields
/// are derived from `FX`/`FY` whenever a `LineGeometry` consumes the spacing,
/// so the lanes differ only in what the *object* reports between the `Like=`
/// and the next recalculation; no golden or gated corpus deck reads it there.
pub const LINESPACING_MAKELIKE_DROPS_EQUIV_SPACING_PARITY_IMPL: bool = true;
/// See the parity twin above.
pub const LINESPACING_MAKELIKE_DROPS_EQUIV_SPACING_DEFAULT_IMPL: bool = false;

#[cfg(not(feature = "oracle-parity"))]
pub use LINESPACING_MAKELIKE_DROPS_EQUIV_SPACING_DEFAULT_IMPL as LINESPACING_MAKELIKE_DROPS_EQUIV_SPACING;
#[cfg(feature = "oracle-parity")]
pub use LINESPACING_MAKELIKE_DROPS_EQUIV_SPACING_PARITY_IMPL as LINESPACING_MAKELIKE_DROPS_EQUIV_SPACING;

/// Whether a **LineCode**'s `C0=` fails to select the symmetrical-component
/// model.
///
/// `true` reproduces the upstream quirk: `TLineCodeObj.PropertySideEffects`
/// lists `R1 X1 R0 X0 C1 B1 B0` — every sequence quantity except `C0` — as the
/// properties that set `SymComponentsModel := TRUE` and clear the matrix
/// property tracking. The omission is a slip the Pascal source itself flags
/// with a `-- Missing?` comment. So `New LineCode.x nphases=3 c0=…` alone
/// leaves the object on the *matrix* model and the value inert, while the same
/// edit spelled `c1=` switches models.
///
/// `false` includes `C0` in that list — the clean fix the upstream comment
/// itself asks for. The lanes differ only on a LineCode edit whose **only**
/// sequence property is `C0`; every golden and gated corpus deck that sets
/// `C0` also sets `R1`/`X1`/`C1`, which already select the model.
pub const LINECODE_SYM_CLEAR_OMITS_C0_PARITY_IMPL: bool = true;
/// See the parity twin above.
pub const LINECODE_SYM_CLEAR_OMITS_C0_DEFAULT_IMPL: bool = false;

#[cfg(not(feature = "oracle-parity"))]
pub use LINECODE_SYM_CLEAR_OMITS_C0_DEFAULT_IMPL as LINECODE_SYM_CLEAR_OMITS_C0;
#[cfg(feature = "oracle-parity")]
pub use LINECODE_SYM_CLEAR_OMITS_C0_PARITY_IMPL as LINECODE_SYM_CLEAR_OMITS_C0;

// The **GICTransformer `G2` off `%R1`** row belongs here by shape but is NOT
// split: the flip was implemented, gated and reverted in F.3k because it is not
// gate-invisible. `tests/corpus/asymmetric/gic/gictransformer_gic.dss:18` builds
// `GICTransformer.tg3 … %R1=0.2 %R2=0.15`, so honouring `%R2` moves that deck's
// GIC current 4.50e-4 against `capi_v0145` (allowed 1.00e-6) and `gic_midi.dss`'s
// 1.02e-4 (allowed 1.07e-6). Both gating oracles reproduce the quirk, so the fix
// costs those two decks' primary physical channel in the default lane — the
// Newton-row shape (a field-scoped exclusion + a replacement in-engine assertion
// + a transitive cover), which is a commit of its own. The site keeps its marker
// and its reproduction pin; see `elements/pd/gic_transformer/solve.rs`.

/// Whether `Export SeqCurrents` prints a non-positive current rating **raw** in
/// its `%Normal`/`%Emergency` percentage columns.
///
/// `true` reproduces the upstream quirk: `ExportResults.pas:409-414` seeds
/// `iNormal := NormAmps` and only *overwrites* it with `I1/NormAmps*100` when
/// the rating is `> 0`, so an undefined or negative rating leaks the rating
/// itself into a column whose header says "percent" (`normamps=-1` prints
/// `-1`).
///
/// `false` prints `0` for an undefined rating — the clean fix named at the
/// site, and the only value that keeps the column's declared meaning. The lanes
/// differ only for an element with a non-positive `normamps`/`emergamps`; every
/// golden and gated corpus deck rates every element positively, which is why
/// the quirk was marked "unpinnable" at the site until now.
pub const SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING_PARITY_IMPL: bool = true;
/// See the parity twin above.
pub const SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING_DEFAULT_IMPL: bool = false;

#[cfg(not(feature = "oracle-parity"))]
pub use SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING_DEFAULT_IMPL as SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING;
#[cfg(feature = "oracle-parity")]
pub use SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING_PARITY_IMPL as SEQ_CURRENTS_PRINTS_RAW_NONPOSITIVE_RATING;

/// Whether the short-line **merge-with-parent** reduction inspects only the
/// parent branch's *first* shunt when looking for a capacitor/reactor.
///
/// `true` reproduces the upstream quirk: `DoReduceShortLines`
/// (`ReduceAlgs.pas:200-210`) opens the scan with `ParentNode.FirstShuntObject()`
/// but advances it with `PresentBranch.NextShuntObject()` — a cross-node cursor
/// mix. The present branch's `TDSSPointerList` cursor still sits at its last
/// item from tree construction (`Add` sets `ActiveItem := Count`,
/// `DSSPointerList.pas:66`), so the very first `Next` overflows and returns
/// `NIL` (`:113-131`), ending the loop after one element. A capacitor or
/// reactor at parent-shunt position ≥ 2 therefore fails to block the merge and
/// is silently moved to another bus.
///
/// `false` scans every parent shunt. That the **merge-with-child** branch of the
/// same procedure (`:246-258`) spells the identical loop with a single cursor
/// (`PresentBranch.First…`/`PresentBranch.Next…`) — and that this port already
/// renders it as an `any()` — is what makes the parent branch a slip rather than
/// a rule.
///
/// The lanes differ only when a parent branch carries ≥ 2 shunts whose *first*
/// is not a capacitor/reactor while a later one is; no golden and no gated
/// corpus deck reduces such a topology.
pub const REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT_PARITY_IMPL: bool = true;
/// See the parity twin above.
pub const REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT_DEFAULT_IMPL: bool = false;

#[cfg(not(feature = "oracle-parity"))]
pub use REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT_DEFAULT_IMPL as REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT;
#[cfg(feature = "oracle-parity")]
pub use REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT_PARITY_IMPL as REDUCE_SCANS_ONLY_THE_FIRST_PARENT_SHUNT;

/// Whether the StorageController's "is the fleet already idling?" test is
/// written as a **bitwise complement** of the state ordinal.
///
/// `true` reproduces the upstream quirk: `TStorageControllerObj` guards both of
/// its terminal branches with `if not FleetState = STORE_IDLING`
/// (`StorageController.pas:1350` in the discharge path, `:1619` in the charge
/// path). In Object Pascal `not` binds tighter than `=`, and `FleetState` is an
/// `Integer`, so this parses as `(not FleetState) = 0` — a bitwise complement,
/// true only for `FleetState = -1 = STORE_CHARGING`. The branch it guards
/// ("Ran out of OOMPH" / "Fully charged") therefore fails to idle the fleet in
/// exactly the state that reaches it: a fleet that runs out of energy *while
/// discharging* is left discharging, and only a *charging* fleet is idled.
///
/// `false` asks the question the code reads as: `FleetState <> STORE_IDLING`.
/// That the two sites' own comments (`// force a new power flow solution`) and
/// the `SetFleetToIdle` call they guard describe an unconditional
/// idle-unless-already-idle is what makes it a precedence slip rather than a
/// convention.
///
/// The lanes differ only for a fleet that reaches "out of OOMPH"/"fully charged"
/// in a non-idle, non-charging state; the parity lane keeps it, so every gating
/// oracle comparison is unchanged there.
pub const STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL_PARITY_IMPL: bool = true;
/// See the parity twin above.
pub const STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL_DEFAULT_IMPL: bool = false;

#[cfg(not(feature = "oracle-parity"))]
pub use STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL_DEFAULT_IMPL as STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL;
#[cfg(feature = "oracle-parity")]
pub use STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL_PARITY_IMPL as STORAGE_CONTROLLER_IDLE_TEST_COMPLEMENTS_THE_ORDINAL;

/// Whether `Export Storage_Meters /m` names its per-element files with the
/// **PVSystem** prefix.
///
/// `true` reproduces the upstream quirk: `WriteMultipleStorageMeterFiles`
/// (`ExportResults.pas:2240`) was cloned from the PVSystem writer and kept its
/// `'EXP_PV_'` literal, so a Storage fleet's per-element registers land in
/// `EXP_PV_<NAME>.csv` — colliding with the PVSystem export's own files
/// whenever both are written into one directory.
///
/// `false` uses `EXP_STORAGE_`, which is what the single-file sibling of the
/// same command already writes (`EXP_STORAGEMeters.csv`) and what the PVSystem
/// writer's own prefix implies. Only the *file name* moves; the rows are
/// byte-identical, and the single-file path is untouched in both lanes.
pub const STORAGE_MULTIFILE_USES_THE_PV_PREFIX_PARITY_IMPL: bool = true;
/// See the parity twin above.
pub const STORAGE_MULTIFILE_USES_THE_PV_PREFIX_DEFAULT_IMPL: bool = false;

#[cfg(not(feature = "oracle-parity"))]
pub use STORAGE_MULTIFILE_USES_THE_PV_PREFIX_DEFAULT_IMPL as STORAGE_MULTIFILE_USES_THE_PV_PREFIX;
#[cfg(feature = "oracle-parity")]
pub use STORAGE_MULTIFILE_USES_THE_PV_PREFIX_PARITY_IMPL as STORAGE_MULTIFILE_USES_THE_PV_PREFIX;
