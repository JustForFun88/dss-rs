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
//! | single-point stddev | *torn down* (GOLDEN_REBASE G2.1a) — `support::mathutil` returns `0.0` in both lanes | — |
//! | RPN pi | `dss-parser` `compat::PI` | **yes** (F.3d) |
//! | FPC round | `dss-parser` `compat::round_i32` | **yes** (F.3a) |
//! | solver execution (`Par`, refinement) | `dss-sparse` `compat` | no — declaration only, M3c / WP-R1 own the flip |
//! | Y triplet dedup | *no split* — one shared kernel serves both lanes (IV.1) | — |
//! | sym components | *no split* — measured, see below | — |
//! | Export SeqCurrents `Iresidual` | *torn down* (GOLDEN_REBASE G2.2a) — `report::export::seq_currents` sums the row's own terminal in both lanes | — |
//! | multi-meter `Bus_Int_Duration` | *torn down* (GOLDEN_REBASE G2.2a) — `solution::meters::reliability` walks only its own zone in both lanes | — |
//! | Monitor `BaseFrequency` 60.0 (CLAUDE.md bug 6) | *torn down* (GOLDEN_REBASE G2.2b) — `exec::command::create_object_no_edit` inherits `Fundamental` for every element, Monitor included, in both lanes | — |
//! | Newton stale `Iterminal` in Powers/Losses (CLAUDE.md bug 5) | *torn down* (GOLDEN_REBASE G2.3) — `exec::view::snapshot_elements` recomputes `Iterminal` at the converged `NodeV` for Powers, Losses and Currents alike, in both lanes | — |
//! | report text rendering — number formats (`%g`, script fixed-point, JSON float + line break) | the *Report text rendering* section below | **yes** (F.4a) |
//! | report text rendering — `Show` device-name column width | [`max_device_name_length`] — same section | **yes** (F.4b) |
//! | single-site upstream quirks (`PORTING_PLAN` §4.1 rule 4) | the *Single-site upstream quirks* section below | **partly** (F.3k, F.3l…, F.3w); the section shrinks row by row as `GOLDEN_REBASE_PLAN.md` WP-G2 tears them down — CapControl `Like=` was G2.1b, the `Export SeqCurrents` non-positive rating G2.1c, the short-line merge's parent-shunt scan G2.1d, the StorageController idle guard G2.1e, the Storage `/m` export prefix G2.1f, the CIM wye `grounded` flag G2.1g, the Line height-unit re-read G2.1h, the Isource `Bus2` latch G2.2b, the two CIM attribute names and the Fault `Dump` `MinAmps` reprint G2.2c, the two Relay event-log labels G2.2d, and the unflushed monitor-channel pad G2.4 — that last one mostly *reclassified* rather than fixed: the `[0.0]` it reproduced comes from the oracle clients' stream decoder, not from an engine (the engines' own answer there, `SampleCount` fabricated zeros, is a separate upstream defect this port declines, unobservable through either client) |
//!
//! The Monitor `BaseFrequency` and Newton stale-`Iterminal` rows were not in
//! IV.2's table and did not extend it: they *were* the two **reproduced**
//! CLAUDE.md upstream bugs whose clean fix was deferred to this pass — Monitor
//! **by name**, in that document's own bug bullet; Newton under
//! `PORTING_PLAN.md` §4.1 rule 4's blanket deferral, quoted ten lines below,
//! which is what sanctioned it. (Named here, not numbered: the table above loses
//! a row per teardown, so an ordinal goes stale on its own. And CLAUDE.md's
//! Newton bullet named no Stage F deferral until F.3j wrote one; the earlier
//! claim that both were deferred "by name" overreached.) With them the
//! named-bug set is closed, and since `GOLDEN_REBASE_PLAN.md` G2.3 it is closed
//! in the strong sense — **no** CLAUDE.md upstream bug is reproduced in either
//! lane. Of the six, two were never reproduced at all (VSConverter's
//! self-aliased `MVMult`, harmonics `Powers`-after-`Currents`) — as is the
//! out-of-range half of `Bus_Int_Duration`. The four that *were* reproduced are
//! all gone: G2.2a tore down `Iresidual` and the in-range `Bus_Int_Duration`
//! cross-zone overwrite, G2.2b tore down Monitor `BaseFrequency`, and G2.3 tore
//! down the Newton stale `Iterminal`.
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
//! build the same circuits. That figure came from a one-off instrumented build
//! and is a *survey*, not the verdict's basis — what the verdict rests on is
//! re-runnable in-tree: `tests::dense_inverse_kernels_differ_by_one_ulp_on_an_
//! ideal_switch` and `tests/compat_dense_inverse.rs`.
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
//! Measured against the correctly-rounded quotient — an **exact-rational**
//! reference over 20 000 operand pairs spanning 1e-6…1e6 in both Smith
//! branches, re-runnable as `python tools/lanes/cdiv_sweep.py sweep`
//! (re-measured 2026-08-01):
//!
//! | kernel | mean rel. error | worst rel. error | outside `|den|` ∈ [1e-154, 1e154] |
//! |---|---|---|---|
//! | Smith (`cdiv_fpc_impl`) | **5.68e-17** | **3.68e-16** | still exact |
//! | naive (`cdiv_std_impl`) | 7.73e-17 | 4.50e-16 | `0` or `NaN` — total loss |
//!
//! The 2026-07-27 figures (9.42e-17 / 3.82e-16 against 1.05e-16 / 4.26e-16)
//! took their reference from the operands' *decimal spellings* rather than
//! their binary values; same ordering, different magnitudes. Note also that the
//! verdict is the **aggregate**: on ~25% of individual pairs Smith is the
//! worse of the two, so a per-pair claim would be false.
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

// The **Newton stale `Iterminal`** row is gone (`GOLDEN_REBASE_PLAN.md` G2.3).
// `DoNewtonSolution`'s final `SumAllCurrents` stamps every element's
// `Iterminal` from the *pre-final* voltage guess `NodeV_{n-1}` and marks it
// solved for the current `SolutionCount` (`Common/Solution.pas:944-948`, the
// `NodeV -= dV` update at `:965-968`; r4133
// `Version8/Source/Common/Solution.pas` is the same order), so upstream's
// cache-aware `Get_Powers`/`Get_Losses` (`CktElement.pas:542-550`) return a
// one-Newton-step-stale current while `CktElement.Currents` recomputes fresh at
// `NodeV_n` — one read of one element reporting `S != V·conj(I)`, the identity
// `Powers` is *defined* by. Both lanes now recompute all three reads at the
// converged `NodeV` (`exec::view::snapshot_elements`), which is what every
// other algorithm already got: after a fixed-point / direct / harmonic solve
// the cache is invalid at read time, so nothing but a Newton solve moves.
//
// Its observable is the two gated `modes/newton/` decks' element
// powers/losses, which no oracle channel reports correctly (the quirk is in
// v9.8/r3723, v10.2/r4088 and v11.0/r4133 alike, fingerprint 0.478 kVA, checked
// 2026-07-08): `harness::lane::LANE_SKIP_ELEM_POWERS` now excludes those two
// channels in **both** lanes, and `exec::tests::newton` carries the replacement
// — the in-engine dispatch tripwire plus the expected-value pin that Newton's
// powers equal the normal algorithm's. See
// `investigations/issue-05-newton-stale-iterminal.md`.

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

// The **Isource `Bus2` never latches** row was the first member of this
// section and is gone: `GOLDEN_REBASE_PLAN.md` G2.2b tore it down, so
// `elements::pc::isource`'s `BUS2` side effect latches `bus2_defined` in both
// lanes, like `TVsourceObj.PropertySideEffects` (`Vsource.pas:498`; r4133
// `Version8/Source/PCElements/Vsource.pas:468`) always did.

// The **GICTransformer `G2` off `%R1`** row belongs here by shape but is NOT
// split: the flip was implemented and gated in F.3k, and re-measured in F.3v,
// which corrects what the cost actually is. `type=Auto` puts the G1 and G2
// blocks in series on the H→X→neutral path, so honouring `%R2` changes the
// element's admittance, the system Y and the node voltages — the channel both
// runs abort on is the **node voltages** (`gictransformer_gic.dss` 4.502e-4 vs
// an allowed 1.001e-6; `gic_midi.dss` 1.021e-4 vs 1.074e-6), not the "GIC
// current" F.3k named. The treatment is therefore a **whole-case** default-lane
// exclusion — not the Newton row's field-scoped one — costing two of 520 gated
// cases their entire default-lane oracle comparison, including their unrelated
// GICLine/GICsource surface. That is an owner decision, so the site keeps its
// marker and its reproduction pin; see `elements/pd/gic_transformer/solve.rs`
// for the full measurement and for the `R1=`/`R2=` transitive cover it leaves
// ready for whoever lands it.

// The two **CIM attribute-name** rows are gone the same way
// (`GOLDEN_REBASE_PLAN.md` G2.2c): the delta shunt arm writes
// `ShuntCompensator.grounded` — the name its own wye sibling six lines above
// uses and the only class CIM100 declares `grounded` on — and the
// symmetrical-components line writer closes its `bch`/`gch`/`b0ch` quartet with
// `ACLineSegment.g0ch`, the name the `PerLengthSequenceImpedance` sibling
// spells. Both lanes now write them; `tests/golden_cim.rs` applies the two
// renames to the oracle goldens unconditionally, so no golden byte moved.

// The two **Relay event-log** rows left together
// (`GOLDEN_REBASE_PLAN.md` G2.2d), both of them label-only — no computed value
// moves in either lane:
//
// * the per-`Sample` state trace. `TRelayObj.Sample` closes its `FPresentState`
//   resync with a bare `AppendtoEventLog('Debug Sample: Relay.' + Name,
//   'FPresentState: …')` (r4133 `Version8/Source/Controls/Relay.pas:1325`) — no
//   `if DebugTrace`, and not gated on `ShowEventLog` either, so every relay
//   writes one debug line per control sample into the *user-facing* log. The
//   Recloser's byte-identical line **is** guarded (`Recloser.pas:1044`), and so
//   is every other `Debug Sample` line in Relay.pas itself (`:1822`, `:1845`);
//   exactly one line lost its guard, and r4088 did not have the line at all.
//   Both lanes now route it through the `Relay::dbg` helper the port already
//   had, so it is written when — and only when — `DebugTrace` says so.
// * the operation-count reset label. Both `CTRL_RESET` arms of
//   `TRelayObj.DoPendingAction` write `'Recloser.' + Self.Name`
//   (`Relay.pas:1196`, `:1212`), a verbatim copy of `Recloser.pas:909`/`:924`,
//   while all eight other events of that same procedure (`:1087`-`:1176`) write
//   `'Relay.' + Self.Name` — and both earlier revisions of this code (r4088
//   `Relay.pas:971`, dss_capi 0.14.5 `Relay.pas:1003`) label it correctly. Both
//   lanes now name the class that emitted the event.
//
// `harness::lane::expected_eventlog` applies both rewrites to the oracle
// capture in **both** lanes, so the gated `oracle: "r4133"` protection decks
// keep comparing every other line against the oracle and no golden byte moved.

// The **Fault `Dump` reprints `MinAmps`** row left with the two CIM ones
// (`GOLDEN_REBASE_PLAN.md` G2.2c): `TFaultObj.DumpProperties` starts its generic
// tail at `NumPropsThisClass` = `Ord(High(TProp))` = 9 = `MinAmps` itself
// (`Fault.pas:533`; r4133 `Version8/Source/PDElements/Fault.pas:594` with
// `NumPropsthisclass = 9` `:107`), so the loop's first iteration re-emits the
// property the custom `~ MinAmps=%.1f` line just wrote. Every other class with
// that loop writes `NumPropsThisClass + 1` (`Transformer.pas:1276`,
// `AutoTrans.pas:1307`, `XfmrCode.pas:663`), so both lanes now start the tail at
// `NormAmps`; `golden_reports`' `fault_dump_expected` drops the second of each
// pair from the oracle goldens unconditionally, so no golden byte moved.

// `MONITOR_CHANNEL_PADS_THE_UNFLUSHED_STREAM` lived here until
// `GOLDEN_REBASE_PLAN.md` G2.4 **reclassified** it — the only row this module
// ever carried whose upstream was not Pascal. The `[0.0]` the parity lane used
// to emit for a header-only monitor stream is fabricated by the **client layer
// of both oracle channels**, never by an engine:
//
// * dss-python's `IMonitors.Channel` (`dss/IMonitors.py:28-55`) does not call
//   `Monitors_Get_Channel` at all: it pulls the raw `ByteStream` and
//   short-circuits `if cnt == 272: return np.zeros((1,), dtype=np.float32)`,
//   272 being the header-only stream size.
// * our own r4133 bridge reproduces that decoder by design
//   (`crates/dss-epri/src/dss.rs:625-634`, "exactly like dss-python",
//   `cnt == 272 -> vec![0.0]`) — and, since it is what the `r4133` channel's
//   captures are actually made of, it is the load-bearing evidence for that
//   channel, not any Pascal accessor.
//
// **Corrected 2026-08-06 (audit).** The engines are not innocent here, they are
// simply never reached through those two clients. `Monitors_Get_Channel` keeps
// its empty `DefaultResult` (`CAPI_Monitors.pas:304`) only for `SampleCount <=
// 0` (`:308`) or an invalid index (`:313-320`); with samples taken and nothing
// flushed — `TakeSample` increments `SampleCount` (`Monitor.pas:1195`) while
// only `Save` grows the stream (`:1122-1125`) — it returns `SampleCount` zeros
// read out of a zero-filled `AllocMem` buffer whose reads all fail at EOF
// (`:321-330`). r4133's native accessor pads `myDBLArray := [0]` only while
// `SampleCount = 0` and otherwise walks the same unwritten region
// (`Version8/Source/DDLL/DMonitors.pas:509-541`). So no official reader reports
// an empty channel, but neither engine reports `[0.0]` either: the placeholder
// is purely a client artifact, and the `SampleCount` zeros are an upstream
// defect this port does not reproduce (2026-08-02 policy). Neither is
// observable through a gating client, which is why the row owes no ledger
// entry.
//
// Hence both lanes report the empty channel
// (`elements::meter::monitor::Monitor::channel`, which thereby also agrees with
// its own `dbl_hour`) and the client placeholder is normalized out of the
// capture — lane-independently **and** channel-independently, because both
// gating channels' readers fabricate it
// (`harness::lane::expected_monitor_channel`).

/// The scale `CalcVoltageBases` applies to a bus's solved L-N magnitude before
/// searching the legal-base list — the **truncated `√3/1000`** of upstream's
/// own statement.
///
/// [`kv_base_search_scale_truncated_impl`] (`0.001732`) reproduces the literal:
/// `SetVoltageBases` computes
/// `kVBase := NearestBasekV(Cabs(NodeV^[GetRef(1)]) * 0.001732) / SQRT3`
/// (`Common/Solution.pas:1103`; the identical statement in r4133
/// `Version8/Source/Common/Solution.pas:2541`). `SQRT3` there is the
/// full-precision `Sqrt(3.0)` the unit computes at startup, so **one statement
/// carries `√3` twice** — truncated on the way in, exact on the way out — and
/// the estimate it searches with is 2.93e-5 relative *low*. That is the
/// section's rule (ii) satisfied by the source itself: the constant the author
/// meant is named, at full precision, in the same line.
///
/// [`kv_base_search_scale_exact_impl`] is that named constant, `SQRT3 / 1000`.
///
/// **What the two lanes can move is *which base is picked*, and nothing else.**
/// The scaled estimate is never stored: it is consumed by the relative-distance
/// argmin `nearestBasekV` (`|1 − kv/base|` over `Set VoltageBases`), and the bus
/// records `matched / SQRT3` — a number taken verbatim from the user's list, not
/// from the estimate. So the lanes write **bit-identical** `kVBase` for every
/// bus whose estimate is not within 2.93e-5 of a tie between two adjacent legal
/// bases (the tie of `a` and `b` in this metric being their harmonic mean
/// `2ab/(a+b)`). Measured: with the flip selected, every golden and all 520
/// gated corpus cases are unchanged in the default lane — the row moves no
/// oracle-compared number anywhere in the suite.
///
/// The divergence is pinned where it is observable rather than narrated:
/// `solution::solution::dispatch::tests` drives the argmin at a constructed tie
/// (both scales, both outcomes) and then the whole `CalcVoltageBases` command
/// through a deck built to sit in that window, asserting the per-lane `kVBase`.
pub fn kv_base_search_scale_truncated_impl() -> f64 {
    0.001732
}

/// See the parity twin above — the full-precision `SQRT3 / 1000` upstream's own
/// statement names one operator later.
pub fn kv_base_search_scale_exact_impl() -> f64 {
    crate::util::sqrt3() / 1000.0
}

#[cfg(not(feature = "oracle-parity"))]
pub use kv_base_search_scale_exact_impl as kv_base_search_scale;
#[cfg(feature = "oracle-parity")]
pub use kv_base_search_scale_truncated_impl as kv_base_search_scale;

/// The divisor `Export Profile` uses to turn a **line-to-line** volt magnitude
/// into per-unit: upstream's truncated `1000·√3`.
///
/// [`profile_ll_pu_divisor_truncated_impl`] reproduces the literal `1732.0`
/// (`Common/ExportResults.pas:3207/3231/3256`; r4133
/// `Version8/Source/CMD_Lazz/Common/ExportResults.pas:2979/2995/3012`, so both
/// gating oracles carry it). `Bus.kVBase` is the **line-to-neutral** base kV, so
/// the L-L base is `√3` times it and the correct divisor is `1000·√3 =
/// 1732.0508…`; the literal is 2.93e-5 relative *low*, which makes every
/// reported L-L per-unit 2.93e-5 relative *high*.
///
/// **The section's rule (ii) is satisfied inside the same procedure, by the
/// sibling branch.** `ExportProfile`'s line-to-neutral arms divide by the exact
/// `1000.0` (eight sites, e.g. `:3152`/`:3167`) while its three line-to-line
/// arms divide by the four-digit `1732.0` — one routine, one quantity, one
/// branch exact and the other truncated. Nothing else in the procedure differs,
/// so the intended constant is named by the code next to it, exactly as
/// [`kv_base_search_scale`]'s `SQRT3` is named one operator later.
///
/// [`profile_ll_pu_divisor_exact_impl`] is that constant.
///
/// **Reach: one number, in one report.** `pu_ll` is local to
/// `report::export::profile` and feeds only the `puV1`/`puV2` column of the
/// `Export Profile` L-L variants (`ll3ph`, `llall`, `llprimary`). It reaches no
/// solve, no `Y`, no element state and no other report — measured: with the flip
/// selected the *only* moving assertion in the whole suite is
/// `export_profile_ll3ph` row 0 field 2 (`1.04672` vs the oracle's `1.04675`),
/// and all 520 gated corpus cases are unchanged. That is why this row is the
/// drift model's "excluded from oracle comparison **at those fields**" and not a
/// whole-artifact escape: the default lane keeps byte/parsed gating on every
/// other column of the same goldens, and on the L-N variants of the same report
/// in full.
pub fn profile_ll_pu_divisor_truncated_impl() -> f64 {
    1732.0
}

/// See the parity twin above — the exact `1000·√3` the sibling L-N branch's
/// `1000.0` names.
pub fn profile_ll_pu_divisor_exact_impl() -> f64 {
    1000.0 * crate::util::sqrt3()
}

#[cfg(not(feature = "oracle-parity"))]
pub use profile_ll_pu_divisor_exact_impl as profile_ll_pu_divisor;
#[cfg(feature = "oracle-parity")]
pub use profile_ll_pu_divisor_truncated_impl as profile_ll_pu_divisor;

// The **LoadShape MMF plain-text accept-set** row belongs here by shape but is
// NOT split, and F.3v measured why rather than assuming it. The quirk is real
// and its witness is as strong as this section's rule asks for — `TLoadShapeObj`
// owns a *second* reader for the same format, `ReadCSVFile`'s non-mapped branch
// (`LoadShape.pas:1044`), which parses each row with the aux parser and so
// honours the sign and the exponent the mapped branch deletes (`:1374`). But
// `tests/corpus/modes/inputformat/shape_mmf/shape_mmf.dss` exists *to observe*
// the quirk: its `mmpq8.csv` P column is written in exponent notation on
// purpose, so honouring it moves that deck's node voltages by 1.641e1 V against
// an allowed 8.179e-6. Whole-case default-lane exclusion again — including the
// deck's unrelated sng/dbl/`mult=(sngfile=)` MMF-reader coverage — so the row
// keeps its marker; see `elements/general/load_shape/compute.rs`.

// ---------------------------------------------------------------------------
// Report text rendering — the F-FMT seam (IV.2 row 11, step F.4)
// ---------------------------------------------------------------------------
//
// `DE_PASCALIZE_PLAN.md` Part IV.2 §F-FMT step 1: *every* number-to-text call
// goes through an alias declared here, so the lane makes one decision per
// rendering rule instead of one per call site. The parity kernels are the FPC
// RTL emulations moved as-is; the default kernels are native `format!` with
// explicit precision.
//
// **What F-FMT is allowed to move, and what it is not.** The rows below change
// how a number is *spelled* — never which number it is, never a column set,
// never a row order. IV.1 keeps `Export`/CSV column sets and order, `Save`
// re-compilability and monitor channel precision contractual, and the default
// kernels below preserve digit counts and notation windows precisely so the
// fixed-width tables keep their columns. What they drop is FPC's *two-stage
// decimal re-rounding*, which is the one thing in this family that is an
// inexactness rather than a layout policy.
//
// **Why the default lane's goldens still gate.** §F-FMT step 3: the byte-golden
// families are compared through the already-existing parsed-numeric tokenizer
// (`tests/harness/lane.rs::compare_report`) against the *same committed
// goldens*, at `rel = abs = 0`. So a re-spelled number passes only while it
// parses to the identical `f64`; a changed value, a changed column count or a
// changed row order fails in both lanes. F.4 opens no re-baseline event.
//
// **`comma_text` — declared, and measured to need *no* split.** IV.2's table
// bundles FPC `TStrings.CommaText` into the parity kernel. Read at the source
// (`crate::util::comma_text`), its rule is "quote an item containing any char
// `<= ' '`, the delimiter, or the quote char; double an embedded quote" — which
// is the RFC-4180 quoting rule restricted to this alphabet, not an FPC wart:
// a native CSV writer produces the identical bytes for every monitor header
// label the engine can emit. It carries no compat marker for the same reason.
// Splitting it would therefore select the same behaviour twice, so it stays one
// shared kernel — same disposition as "Y triplet dedup", reached the same way.

/// General (`%g`) number rendering — the seam every report, dump, event-log and
/// trace line lands in ([`crate::util::fmt_g`]).
///
/// * parity — [`crate::util::fmt_g_fpc_impl`]: the FPC 3.2.2 Grisu1 +
///   `ffGeneral` pipeline, whose final cut to `sig` digits re-rounds a decimal
///   string **half-away-from-zero** on top of an already-rounded 17-digit form.
/// * default — [`crate::util::fmt_g_native_impl`]: the same digit count and the
///   same fixed-vs-scientific window, produced by **one** correctly-rounded
///   conversion.
///
/// Pinned at its observable by `crates/dss-core/tests/fmt_battery.rs`, which
/// asserts the parity kernel byte-for-byte against the real FPC RTL in *both*
/// lanes and measures the exact population where the default kernel disagrees.
#[cfg(feature = "oracle-parity")]
pub use crate::util::fmt_g_fpc_impl as fmt_g;
/// See the parity-lane twin above.
#[cfg(not(feature = "oracle-parity"))]
pub use crate::util::fmt_g_native_impl as fmt_g;

/// See the parity-lane twin above.
#[cfg(not(feature = "oracle-parity"))]
pub use crate::report::format::fixed_w as fixed_w_script;
/// Fixed-point rendering of a number embedded in **emitted DSS script** — the
/// AltDSS whole-circuit `PostCommands` (`Set ueweight=%8.2f`,
/// `Set lossweight=%8.2f`, `CAPI_Obj.pas:2593-2594`).
///
/// * parity — [`crate::report::format::fixed_w_fpc_impl`]: FPC's two-stage
///   rounding (render at 15 significant digits, then round *that decimal*
///   ties-away-from-zero).
/// * default — [`crate::report::format::fixed_w`]: Rust's single correctly-
///   rounded `{:.N}`, which is what the fixed-width `Show` tables already use in
///   both lanes.
///
/// Pinned by `report::format::tests::fixed_w_script_is_the_lane_kernel`.
#[cfg(feature = "oracle-parity")]
pub use crate::report::format::fixed_w_fpc_impl as fixed_w_script;

/// See the parity-lane twin above.
#[cfg(not(feature = "oracle-parity"))]
pub use CONTROL_QUEUE_SEC_DIGITS_DEFAULT_IMPL as CONTROL_QUEUE_SEC_DIGITS;
/// How many significant digits a `Show ControlQueue` row prints its `Sec`
/// column with (`ControlQueue.pas:496`, `Format('%d, %d, %-.g, …')`).
///
/// `%-.g` is FPC `ffGeneral` with an **empty** precision after the dot, and the
/// row is unreachable from the executive — a `show controlqueue` after any
/// `solve` sees a drained queue (probe-proven) — so no oracle capture exists to
/// settle what it renders. The parity lane therefore keeps the 6-digit stand-in
/// the port has always used: it is the value the byte contract was written
/// against, and guessing differently now would trade one unverifiable number for
/// another. The default lane does not guess at all — it renders the column at
/// the seam's full [`fmt_g`] precision (15 significant digits, FPC's own
/// `ffGeneral` default when a precision is omitted), which is a *defined*
/// rendering rather than a reconstruction of an unobservable one.
///
/// Pinned by `report::show::diagnostics::tests::control_queue_row_format`.
#[cfg(feature = "oracle-parity")]
pub use CONTROL_QUEUE_SEC_DIGITS_PARITY_IMPL as CONTROL_QUEUE_SEC_DIGITS;

/// The 6-significant-digit stand-in for FPC's `%-.g` — see
/// [`CONTROL_QUEUE_SEC_DIGITS`].
pub const CONTROL_QUEUE_SEC_DIGITS_PARITY_IMPL: usize = 6;
/// FPC `ffGeneral`'s documented default precision — see
/// [`CONTROL_QUEUE_SEC_DIGITS`].
pub const CONTROL_QUEUE_SEC_DIGITS_DEFAULT_IMPL: usize = 15;

/// How a float is spelled in the AltDSS JSON export.
///
/// * parity — [`crate::report::export::json::fpjson_float_fpc_impl`]: fpjson's
///   `TJSONFloatNumber` default, FPC `Str(Double)` — a fixed
///   17-significant-digit scientific literal with a 3-digit zero-padded
///   exponent (`12.47` → `1.2470000000000001E+001`).
/// * default — [`crate::report::export::json::json_float_shortest_impl`]: the
///   shortest literal that round-trips to the same `f64` (`12.47`).
///
/// The *value* is identical either way — the port's own reader
/// (`report::export::json::read::parse_json`) reads both back to the same
/// `f64`, which its `float_literal_is_bit_exact` test asserts on the lane's
/// kernel. What changes is only how a consumer sees it.
///
/// Pinned by `report::export::json::tests::json_float_is_the_lane_kernel`.
#[cfg(feature = "oracle-parity")]
pub use crate::report::export::json::fpjson_float_fpc_impl as json_float;
/// See the parity-lane twin above.
#[cfg(not(feature = "oracle-parity"))]
pub use crate::report::export::json::json_float_shortest_impl as json_float;

/// See the parity-lane twin above.
#[cfg(not(feature = "oracle-parity"))]
pub use JSON_LINE_BREAK_DEFAULT_IMPL as JSON_LINE_BREAK;
/// The line break fpjson's pretty writer puts between members.
///
/// fpjson emits the RTL platform `sLineBreak`, so the oracle's own output — and
/// therefore the byte goldens — carry **CRLF**, because the pinned oracle runs
/// on Windows. The parity lane reproduces that; the default lane emits `\n`, so
/// the document it writes is the same on every platform. Compact mode has no
/// line breaks and is identical in both lanes.
///
/// Pinned by `report::export::json::tests::json_line_break_is_the_lane_kernel`.
#[cfg(feature = "oracle-parity")]
pub use JSON_LINE_BREAK_PARITY_IMPL as JSON_LINE_BREAK;

/// The Windows `sLineBreak` fpjson wrote into the goldens — see
/// [`JSON_LINE_BREAK`].
pub const JSON_LINE_BREAK_PARITY_IMPL: &str = "\r\n";
/// The platform-independent line break — see [`JSON_LINE_BREAK`].
pub const JSON_LINE_BREAK_DEFAULT_IMPL: &str = "\n";

/// See the parity-lane twin above.
#[cfg(not(feature = "oracle-parity"))]
pub use crate::report::show::max_device_name_length_measured_impl as max_device_name_length;
/// The width of the device-name column in the fixed-width `Show` tables — the
/// **table-layout** half of F-FMT (§F-FMT step 2), as opposed to the number
/// formats above.
///
/// * parity — [`crate::report::show::max_device_name_length_zero_impl`]: the
///   pinned 0.14.5 backend returns **0** whatever the element names are, so the
///   column collapses and `Show BusFlow` glues the terminal number onto the
///   quoted name.
/// * default — [`crate::report::show::max_device_name_length_measured_impl`]:
///   the width the Pascal *source* computes, i.e. the column sized from its own
///   content.
///
/// Pinned by
/// `exec::tests::compat_quirks::device_name_column_width_is_the_lane_kernel`.
#[cfg(feature = "oracle-parity")]
pub use crate::report::show::max_device_name_length_zero_impl as max_device_name_length;

/// How a run of fixed-width `Show` rows becomes text — the **table-layout**
/// kernel of F-FMT (§F-FMT step 2), the counterpart of the number formats above.
///
/// * parity — [`crate::report::table::render_rows_pad_impl`]: replays the Pascal
///   primitives (`Pad`, `PadDots`, `Format('%W…')`) cell by cell, so the bytes
///   are those the hand-built `Format` concatenation produced.
/// * default — [`crate::report::table::render_rows_table_impl`]: `comfy-table`
///   with the `NOTHING` preset, sizing every column from its own content.
///
/// The crate was chosen by the plan's own criterion — a `forbid(unsafe_code)`-
/// clean dependency tree. `comfy-table` 7.2's whole closure (itself,
/// `unicode-width`, `unicode-segmentation`) forbids or denies `unsafe_code`,
/// while `tabled` 0.21's mandatory `papergrid` carries real `unsafe` blocks.
///
/// What may move between the lanes is padding, and nothing else:
/// [`crate::report::table::Cell::sep`] refuses a separator that is not
/// whitespace or a comma, so every token lives in a cell and the table kernel
/// cannot drop one. Pinned by `report::table::tests::both_kernels_tokenize_alike`
/// and, at the report level, by the `Show` goldens in both lanes.
#[cfg(feature = "oracle-parity")]
pub use crate::report::table::render_rows_pad_impl as render_rows;
/// See the parity-lane twin above.
#[cfg(not(feature = "oracle-parity"))]
pub use crate::report::table::render_rows_table_impl as render_rows;
