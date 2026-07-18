//! Symmetrical-component transforms — loop-for-loop port of the pieces of
//! r3723 `Shared/mathutil.pas` + `Shared/Ucmatrix.pas` the vendored
//! `IndMach012a.dpr` links (`Phase2SymComp`/`SymComp2Phase`, `SetAMatrix`,
//! `TcMatrix.MvMult`, `TcMatrix.Invert`).
//!
//! Upstream builds the matrices once at `mathutil.pas` unit `initialization`
//! (`:554-569`): `As2p`/`Ap2s` both get `SetAMatrix`, then `Ap2s.Invert` — the
//! phase→sym matrix is the *numeric* inverse produced by `TcMatrix.Invert`,
//! never the analytic 1/3-form. Reproduced verbatim so every last bit matches
//! the native twin.

use crate::cmath::{cabs, caccum, cdiv, cinv, cmplx, cmul, cnegate, csub, Complex, CONE, CZERO};

/// Column-major 3x3 storage, Pascal `TcMatrix.Values` layout:
/// element (i,j), 1-based, lives at `(j-1)*Norder + i` (`Ucmatrix.pas:239`).
type Mat3 = [Complex; 9];

#[inline]
fn idx(i: usize, j: usize) -> usize {
    (j - 1) * 3 + (i - 1)
}

/// Pascal `mathutil.SetAMatrix` (`mathutil.pas:297-311`).
///
/// TODO(compat): `a = (-0.5, 0.866025403)` reproduces upstream's truncated
/// sqrt(3)/2 (= 0.8660254037844386...); the clean fix (full-precision
/// constant) is deferred to the DE_PASCALIZE Stage F sweep together with the
/// engine-side copies of the same constant.
fn set_a_matrix(m: &mut Mat3) {
    let a = cmplx(-0.5, 0.866025403);
    let aa = cmplx(-0.5, -0.866025403);
    // `For i := 1 to 3 Do SetElemSym(1,i,CONE)` — SetElemsym writes (i,j) and,
    // when i<>j, (j,i) (`Ucmatrix.pas:248-253` semantics).
    for i in 1..=3 {
        m[idx(1, i)] = CONE;
        if i != 1 {
            m[idx(i, 1)] = CONE;
        }
    }
    m[idx(2, 2)] = aa;
    m[idx(3, 3)] = aa;
    // `SetElemsym(2,3,a)`
    m[idx(2, 3)] = a;
    m[idx(3, 2)] = a;
}

/// Pascal `TcMatrix.Invert` (`Ucmatrix.pas:144-234`), specialized to Norder=3.
/// Diagonal-pivoted in-place inversion; ported loop-for-loop including the
/// pivot-selection arithmetic (`Cabs` differences) and the final global negate.
fn invert(a: &mut Mat3) {
    const L: usize = 3;
    let mut lt = [0i32; L + 1]; // 1-based, `LT^[j] := 0`
    let mut t1 = cmplx(0.0, 0.0);
    let mut k: usize = 1;

    for _m in 1..=L {
        for ll in 1..=L {
            if lt[ll] != 1 {
                let rmy = cabs(a[idx(ll, ll)]) - cabs(t1);
                if rmy > 0.0 {
                    t1 = a[idx(ll, ll)];
                    k = ll;
                }
            }
        }

        // `IF RMY=0.0 THEN InvertError := 2` — cannot fire for the fixed
        // A-matrix; keep the check as a loud trap instead of a silent skip.
        let rmy = cabs(t1);
        assert!(rmy != 0.0, "indmach012a: TcMatrix.Invert singular A-matrix");

        t1 = cmplx(0.0, 0.0);
        lt[k] = 1;
        for i in 1..=L {
            if i != k {
                for j in 1..=L {
                    if j != k {
                        a[idx(i, j)] = csub(
                            a[idx(i, j)],
                            cdiv(cmul(a[idx(i, k)], a[idx(k, j)]), a[idx(k, k)]),
                        );
                    }
                }
            }
        }

        a[idx(k, k)] = cnegate(cinv(a[idx(k, k)]));

        for i in 1..=L {
            if i != k {
                a[idx(i, k)] = cmul(a[idx(i, k)], a[idx(k, k)]);
                a[idx(k, i)] = cmul(a[idx(k, i)], a[idx(k, k)]);
            }
        }
    }

    for j in 1..=L {
        for kk in 1..=L {
            a[idx(j, kk)] = cnegate(a[idx(j, kk)]);
        }
    }
}

/// Pascal `TcMatrix.MvMult` (`Ucmatrix.pas:110-124`): `b := M·x`, summation
/// in ascending-j `Caccum` order (order preserved for bit parity).
fn mv_mult(m: &Mat3, x: &[Complex; 3]) -> [Complex; 3] {
    let mut b = [CZERO; 3];
    for i in 1..=3 {
        let mut sum = cmplx(0.0, 0.0);
        for j in 1..=3 {
            caccum(&mut sum, cmul(m[idx(i, j)], x[j - 1]));
        }
        b[i - 1] = sum;
    }
    b
}

struct SymMatrices {
    as2p: Mat3,
    ap2s: Mat3,
}

/// Pascal `mathutil.pas` unit `initialization` (`:554-569`).
fn matrices() -> &'static SymMatrices {
    use std::sync::OnceLock;
    static MATS: OnceLock<SymMatrices> = OnceLock::new();
    MATS.get_or_init(|| {
        let mut as2p = [CZERO; 9];
        let mut ap2s = [CZERO; 9];
        set_a_matrix(&mut as2p);
        set_a_matrix(&mut ap2s);
        invert(&mut ap2s);
        SymMatrices { as2p, ap2s }
    })
}

/// Pascal `mathutil.Phase2SymComp` (`mathutil.pas:193-199`): `V012 := Ap2s·Vph`.
/// Result index 0 = zero sequence, 1 = positive, 2 = negative
/// (`TSymCompArray = Array[0..2]`, `IndMach012Model.pas:15`).
pub fn phase2symcomp(vph: &[Complex; 3]) -> [Complex; 3] {
    mv_mult(&matrices().ap2s, vph)
}

/// Pascal `mathutil.SymComp2Phase` (`mathutil.pas:202-208`): `Vph := As2p·V012`.
pub fn symcomp2phase(v012: &[Complex; 3]) -> [Complex; 3] {
    mv_mult(&matrices().as2p, v012)
}
