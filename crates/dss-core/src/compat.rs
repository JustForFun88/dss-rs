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
//! | complex division | [`cdiv`] — this file | no |
//! | dense inverse (`CMatrix::invert`, `etk_invert`) | [`invert`], [`etk_invert`] — this file | no |
//! | single-point stddev | [`stddev_single_point`] — this file | **yes** (F.3b) |
//! | RPN pi | `dss-parser` `compat::PI` | no |
//! | FPC round | `dss-parser` `compat::round_i32` | **yes** (F.3a) |
//! | solver execution (`Par`, refinement) | `dss-sparse` `compat` | no — declaration only, M3c / WP-R1 own the flip |
//! | Y triplet dedup | *no split* — one shared kernel serves both lanes (IV.1) | — |
//! | sym components | *no split* — measured, see below | — |
//! | Export SeqCurrents `Iresidual` | [`IRESIDUAL_FROM_TERMINAL_1`] — this file | **yes** (F.3c) |
//! | multi-meter `Bus_Int_Duration` | [`BUS_INT_DURATION_WALKS_ALL_BUSES`] — this file | **yes** (F.3c) |
//! | Monitor `BaseFrequency` 60.0 (CLAUDE.md bug 6, deferred here by name) | [`monitor_base_frequency`] — this file | **yes** (F.3c) |
//! | report text rendering | F.4 (`F-FMT`) — `compat::fmt` seam | no |
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

/// Idiomatic complex division: `num_complex`'s `/` operator (the naive
/// `(ac+bd)/(c²+d²)` form). Differs from [`cdiv_fpc_impl`] by ≤ 1 ULP per
/// component on well-scaled operands (pinned in `tests`).
#[inline]
pub fn cdiv_std_impl(num: Complex64, den: Complex64) -> Complex64 {
    num / den
}

#[cfg(feature = "oracle-parity")]
pub use cdiv_fpc_impl as cdiv;
// F.1 staging: the default lane still selects the parity impl (bit-neutral
// seam); F.3 flips this to `cdiv_std_impl`.
#[cfg(not(feature = "oracle-parity"))]
pub use cdiv_fpc_impl as cdiv;

// ---------------------------------------------------------------------------
// Dense inverse (IV.2 row 2)
// ---------------------------------------------------------------------------

/// In-place complex inversion, the exact algorithm of Pascal
/// `TcMatrix.Invert`: Gauss-Jordan with pivots chosen by largest-magnitude
/// *unused diagonal*, **no row exchanges**, cross-terms through FPC's Smith
/// division.
///
/// TODO(compat): on a singular pivot the matrix is left partially transformed,
/// exactly like the Pascal code (callers only check the error). Restore-or-zero
/// on failure once the 1:1 port is complete.
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

        // Normalize the pivot row.
        let d = a[(col, col)];
        for j in 0..n {
            a[(col, j)] /= d;
            inv[(col, j)] /= d;
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

#[cfg(feature = "oracle-parity")]
pub use invert_gj_no_exchange_impl as invert;
// F.1 staging: see the note at `cdiv` — F.3 flips this to
// `invert_partial_pivot_impl`.
#[cfg(not(feature = "oracle-parity"))]
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

#[cfg(feature = "oracle-parity")]
pub use etk_invert_gj_no_exchange_impl as etk_invert;
// F.1 staging: see the note at `cdiv` — F.3 flips this to
// `etk_invert_partial_pivot_impl`.
#[cfg(not(feature = "oracle-parity"))]
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
