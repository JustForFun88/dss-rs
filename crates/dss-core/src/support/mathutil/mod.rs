//! Math utilities, port of `Shared/mathutil.pas`.
//!
//! Includes the symmetrical-component transforms (both the upstream-compatible
//! "official" matrices and the better-precision default ones), terminal power
//! helpers, real-matrix inversion (`ETKInvert`), Bessel functions for cable
//! line constants, and the statistics helpers used by loadshapes and Monte
//! Carlo modes.

#[cfg(test)]
mod tests;

mod rng;
pub use rng::FpcRng;

use num_complex::Complex64;

use super::cmatrix::CMatrix;
use crate::compat;

/// Three-phase complex triple (Pascal `Complex3`).
pub type Complex3 = [Complex64; 3];

fn c(re: f64, im: f64) -> Complex64 {
    Complex64::new(re, im)
}

/// Symmetrical-component conversion matrices (Pascal globals `As2p`/`Ap2s`).
///
/// The engine selects between two variants at runtime: the default
/// better-precision pair, and the "official" pair that is numerically
/// compatible with upstream OpenDSS (its inverse is obtained by actually
/// running the matrix inversion, reproducing its rounding).
#[derive(Debug, Clone)]
pub struct SymComp {
    as2p: CMatrix,
    ap2s: CMatrix,
}

impl SymComp {
    /// Better-precision variant (`SetAMatrix` / `SetAMatrix_inv`) — the
    /// engine default (`SelectAs2pVersion(False)`).
    pub fn precise() -> Self {
        let a = c(-0.5, 0.8660254037844387);
        let aa = c(-0.5, -0.8660254037844387);
        let mut as2p = CMatrix::new(3);
        Self::fill_a(&mut as2p, a, aa);

        let a_3 = a / 3.0;
        let aa_3 = aa / 3.0;
        let one_3 = c(1.0 / 3.0, 0.0);
        let mut ap2s = CMatrix::new(3);
        ap2s.set(0, 0, one_3);
        ap2s.set(0, 1, one_3);
        ap2s.set(0, 2, one_3);
        ap2s.set(1, 0, one_3);
        ap2s.set(1, 1, a_3);
        ap2s.set(1, 2, aa_3);
        ap2s.set(2, 0, one_3);
        ap2s.set(2, 1, aa_3);
        ap2s.set(2, 2, a_3);

        Self { as2p, ap2s }
    }

    /// Upstream-compatible variant (`SetAMatrix_official` + `Invert`): the
    /// truncated `sin 60°` literal plus a numerically *inverted* `Ap2s`, which
    /// together reproduce official OpenDSS's rounding.
    ///
    /// **Not a compat site, and not selected in either lane** (Stage F row 3,
    /// resolved as *no split*): `mathutil.pas:548` ends its initialization with
    /// `SelectAs2pVersion(False)`, so the pinned oracle uses [`Self::precise`],
    /// and this pair is reachable upstream only through the
    /// `DSSCompatFlag.BadPrecision` env flag (`CAPI_DSS.pas:315`) that no gating
    /// oracle sets. It is kept compiled — like `compat::cdiv_std_impl` — as the
    /// measured comparison partner that keeps the "no split" verdict asserted
    /// rather than narrated (`tests::sym_comp_official_vs_precise_gap_is_the_
    /// truncated_sin60_constant` pins the 4.50e-10 relative gap). Deleting it
    /// would delete the evidence; see `dss_core::compat`'s module header.
    pub fn official() -> Self {
        let a = c(-0.5, 0.866025403);
        let aa = c(-0.5, -0.866025403);
        let mut as2p = CMatrix::new(3);
        Self::fill_a(&mut as2p, a, aa);
        let mut ap2s = CMatrix::new(3);
        Self::fill_a(&mut ap2s, a, aa);
        ap2s.invert().expect("the A matrix is never singular");
        Self { as2p, ap2s }
    }

    fn fill_a(m: &mut CMatrix, a: Complex64, aa: Complex64) {
        let one = c(1.0, 0.0);
        m.set(0, 0, one);
        m.set(0, 1, one);
        m.set(0, 2, one);
        m.set(1, 0, one);
        m.set(1, 1, aa);
        m.set(1, 2, a);
        m.set(2, 0, one);
        m.set(2, 1, a);
        m.set(2, 2, aa);
    }

    /// Phase quantities → sequence components: `v012 = Ap2s · vph`
    /// (Pascal `Phase2SymComp`).
    pub fn phase_to_sym(&self, vph: &[Complex64], v012: &mut [Complex64]) {
        self.ap2s.mv_mult(v012, vph);
    }

    /// Sequence components → phase quantities: `vph = As2p · v012`
    /// (Pascal `SymComp2Phase`).
    pub fn sym_to_phase(&self, v012: &[Complex64], vph: &mut [Complex64]) {
        self.as2p.mv_mult(vph, v012);
    }
}

impl Default for SymComp {
    fn default() -> Self {
        Self::precise()
    }
}

/// Total complex power into a terminal: `Σ V[j] · conj(I[j])`
/// (Pascal `TerminalPowerIn`).
pub fn terminal_power_in(v: &[Complex64], i: &[Complex64], nphases: usize) -> Complex64 {
    let mut result = Complex64::ZERO;
    for j in 0..nphases {
        result += v[j] * i[j].conj();
    }
    result
}

/// Power factor of a complex power `S` (Pascal `Utilities.PowerFactor`):
/// `sign(S.re·S.im) · |S.re| / |S|`, or `1.0` if either part is zero (so an
/// unsolved element with `S = 0` reports unity, as the oracle does).
pub fn power_factor(s: Complex64) -> f64 {
    if s.re != 0.0 && s.im != 0.0 {
        let sign = if s.re * s.im < 0.0 { -1.0 } else { 1.0 };
        sign * s.re.abs() / s.norm()
    } else {
        1.0
    }
}

/// Per-conductor complex power in kW/kvar: `kwkvar[j] = V[j]·conj(I[j])·0.001`
/// (Pascal `CalcKPowers`).
pub fn calc_k_powers(kwkvar: &mut [Complex64], v: &[Complex64], i: &[Complex64], n: usize) {
    for j in 0..n {
        kwkvar[j] = v[j] * i[j].conj() * 0.001;
    }
}

/// X/R ratio of an impedance, clamped to ±9999 and defaulting to 9999 for a
/// zero real part (Pascal `GetXR`).
pub fn get_xr(a: Complex64) -> f64 {
    if a.re != 0.0 {
        let result = a.im / a.re;
        if result.abs() > 9999.0 {
            9999.0
        } else {
            result
        }
    } else {
        9999.0
    }
}

/// Parallel combination of two impedances; zero when the sum is exactly zero
/// (Pascal `ParallelZ`).
pub fn parallel_z(z1: Complex64, z2: Complex64) -> Complex64 {
    let denom = z1 + z2;
    if denom.re.abs() > 0.0 || denom.im.abs() > 0.0 {
        (z1 * z2) / denom
    } else {
        Complex64::ZERO
    }
}

/// In-place inversion of a real square matrix in column-major order (Pascal
/// `ETKInvert`), through the Stage F lane seam: the parity kernel is the same
/// no-row-exchange algorithm as [`CMatrix::invert`], the default kernel pivots
/// partially — see [`compat::etk_invert`]. Pascal error code 2 (singular) maps
/// to `Err`.
pub fn etk_invert(a: &mut [f64], norder: usize) -> Result<(), super::cmatrix::SingularMatrix> {
    compat::etk_invert(a, norder)
}

/// Normally distributed random variable via the 12-uniform-sum method
/// (Pascal `Gauss`). `rand` must return uniform samples in `[0, 1)`.
pub fn gauss(mean: f64, std_dev: f64, mut rand: impl FnMut() -> f64) -> f64 {
    let mut a = 0.0;
    for _ in 0..12 {
        a += rand();
    }
    (a - 6.0) * std_dev + mean
}

/// Quasi-lognormal distribution with roughly half the mass below `mean`
/// (Pascal `QuasiLogNormal`).
pub fn quasi_log_normal(mean: f64, rand: impl FnMut() -> f64) -> f64 {
    gauss(0.0, 1.0, rand).exp() * mean
}

/// Sample mean and standard deviation (Pascal `RCDMeanAndStdDev`).
pub fn mean_and_std_dev(data: &[f64]) -> (f64, f64) {
    // A one-element sample has no spread. The Pascal code nevertheless returns
    // the point *itself* as the "standard deviation" (it falls through to the
    // `Mean` assignment and never clears `StdDev`) — the Stage F
    // `stddev_single_point` row: reproduced in the parity lane, `0.0` in the
    // default lane. Same at the three siblings below.
    if data.len() == 1 {
        return (data[0], compat::stddev_single_point(data[0]));
    }
    let n = data.len() as f64;
    let mean = data.iter().sum::<f64>() / n;
    let s: f64 = data.iter().map(|&d| (mean - d) * (mean - d)).sum();
    (mean, (s / (n - 1.0)).sqrt())
}

/// Sample mean and standard deviation over **single-precision** data (Pascal
/// `RCDMeanAndStdDevSingle`, `Shared/mathutil.pas:336`): `Mean` accumulates in
/// f64 over the widened `Single` values, but `S` is a **`Single` accumulator**
/// (each `S := S + Sqr(...)` rounds to f32), the `S / (Ndata-1)` quotient is
/// an f32 division, and FPC resolves `Sqrt(Single)` to the **Single overload**
/// (result rounded to f32 before widening into the `Double` out-param). Every
/// rounding step verified bit-exact against an FPC 3.2.2 x86_64 probe
/// (ppcrossx64; `tools/fpc/single_prec_probe.pas`).
pub fn mean_and_std_dev_single(data: &[f32]) -> (f64, f64) {
    // Single-point sample — see `mean_and_std_dev`.
    if data.len() == 1 {
        return (
            f64::from(data[0]),
            compat::stddev_single_point(f64::from(data[0])),
        );
    }
    let n = data.len() as f64;
    let mean = data.iter().map(|&d| f64::from(d)).sum::<f64>() / n;
    let mut s = 0.0f32;
    for &d in data {
        let diff = mean - f64::from(d);
        s = (f64::from(s) + diff * diff) as f32;
    }
    let q = (f64::from(s) / (n - 1.0)) as f32;
    (mean, f64::from((f64::from(q)).sqrt() as f32))
}

/// Trapezoid-integrated mean/std-dev over **single-precision** data (Pascal
/// `CurveMeanAndStdDevSingle`, `Shared/mathutil.pas:390`). The mean's term is
/// computed **entirely in f32** — FPC types the `0.5` literal `Single` in this
/// all-`Single` product, so `0.5 * (pY[i]+pY[i+1]) * (pX[i+1]-pX[i])` rounds
/// at every step — accumulated in f64 and divided by the **f32** span. The
/// std-dev pass mixes f64 (`dy1`/`dy2` are `Double` locals, promoting `0.5`
/// and the product) with the f32 `dx`/span. Verified bit-exact against the
/// FPC probe (`tools/fpc/single_prec_probe.pas`).
pub fn curve_mean_and_std_dev_single(y: &[f32], x: &[f32]) -> (f64, f64) {
    let n = y.len();
    debug_assert_eq!(x.len(), n);
    // Single-point curve — see `mean_and_std_dev`.
    if n == 1 {
        return (
            f64::from(y[0]),
            compat::stddev_single_point(f64::from(y[0])),
        );
    }
    let mut s = 0.0f64;
    for i in 0..n - 1 {
        let term = 0.5f32 * (y[i] + y[i + 1]) * (x[i + 1] - x[i]);
        s += f64::from(term);
    }
    let span = f64::from(x[n - 1] - x[0]);
    let mean = s / span;

    let mut s = 0.0f64;
    for i in 0..n - 1 {
        let dy1 = f64::from(y[i]) - mean;
        let dy2 = f64::from(y[i + 1]) - mean;
        s += 0.5 * (dy1 * dy1 + dy2 * dy2) * f64::from(x[i + 1] - x[i]);
    }
    (mean, (s / span).sqrt())
}

/// Trapezoid-integrated mean and standard deviation of a curve y(x)
/// (Pascal `CurveMeanAndStdDev`).
pub fn curve_mean_and_std_dev(y: &[f64], x: &[f64]) -> (f64, f64) {
    let n = y.len();
    debug_assert_eq!(x.len(), n);
    // Single-point curve — see `mean_and_std_dev`.
    if n == 1 {
        return (y[0], compat::stddev_single_point(y[0]));
    }
    let mut s = 0.0;
    for i in 0..n - 1 {
        s += 0.5 * (y[i] + y[i + 1]) * (x[i + 1] - x[i]);
    }
    let mean = s / (x[n - 1] - x[0]);

    let mut s = 0.0;
    for i in 0..n - 1 {
        let dy1 = y[i] - mean;
        let dy2 = y[i + 1] - mean;
        s += 0.5 * (dy1 * dy1 + dy2 * dy2) * (x[i + 1] - x[i]);
    }
    (mean, (s / (x[n - 1] - x[0])).sqrt())
}

/// Modified Bessel function I0 by power series, term-for-term identical to the
/// Pascal implementation (used by concentric-neutral cable constants).
pub fn bessel_i0(a: Complex64) -> Complex64 {
    const MAX_TERM: i32 = 1000;
    const EPSILON_SQR: f64 = 1.0e-20;

    let mut result = c(1.0, 0.0); // term 0
    let z_sqr_25 = (a * a) * 0.25;
    let mut term = z_sqr_25;
    result += z_sqr_25; // term 1
    let mut i = 1;
    loop {
        term = z_sqr_25 * term;
        i += 1;
        term /= (i * i) as f64;
        result += term;
        let size_sqr = term.re * term.re + term.im * term.im;
        if i > MAX_TERM || size_sqr < EPSILON_SQR {
            break;
        }
    }
    result
}

/// Modified Bessel function I1 by power series, term-for-term identical to the
/// Pascal implementation.
pub fn bessel_i1(x: Complex64) -> Complex64 {
    const MAX_TERM: i32 = 1000;
    const EPSILON_SQR: f64 = 1.0e-20;

    let mut term = x / 2.0;
    let mut result = term;
    let mut incterm = term;
    let mut i = 4;
    loop {
        let newterm = x / i as f64;
        term *= incterm * newterm;
        result += term;
        incterm = newterm;
        i += 2;
        let size_sqr = term.re * term.re + term.im * term.im;
        if i > MAX_TERM || size_sqr < EPSILON_SQR {
            break;
        }
    }
    result
}

/// NEMA voltage unbalance in percent: max deviation from the average
/// magnitude over the average magnitude (Pascal `PctNemaUnbalance`).
pub fn pct_nema_unbalance(vph: &Complex3) -> f64 {
    let vmag: Vec<f64> = vph.iter().map(|v| v.norm()).collect();
    let vavg = vmag.iter().sum::<f64>() / 3.0;
    let max_diff = vmag.iter().map(|m| (m - vavg).abs()).fold(0.0, f64::max);
    if vavg != 0.0 {
        max_diff / vavg * 100.0
    } else {
        0.0
    }
}

/// Discrete PI controller used by some inverter controls (Pascal `TPICtrl`).
#[derive(Debug, Clone)]
pub struct PiCtrl {
    den: [f64; 2],
    num: [f64; 2],
    pub k_num: f64,
    pub k_den: f64,
    pub kp: f64,
}

impl PiCtrl {
    /// Constants for a rising function of 5 steps, like the Pascal `Create`.
    pub fn new() -> Self {
        Self {
            den: [0.0; 2],
            num: [0.0; 2],
            k_num: 0.8647,
            k_den: 0.1353,
            kp: 0.02,
        }
    }

    /// One controller step (Pascal `SolvePI`).
    pub fn solve_pi(&mut self, set_point: f64) -> f64 {
        self.num[0] = self.num[1];
        self.num[1] = set_point * self.kp;
        self.den[0] = self.den[1];
        self.den[1] = self.num[0] * self.k_num + self.den[0] * self.k_den;
        self.den[1]
    }
}

impl Default for PiCtrl {
    fn default() -> Self {
        Self::new()
    }
}
