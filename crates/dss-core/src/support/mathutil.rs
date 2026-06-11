//! Math utilities, port of `Shared/mathutil.pas`.
//!
//! Includes the symmetrical-component transforms (both the upstream-compatible
//! "official" matrices and the better-precision default ones), terminal power
//! helpers, real-matrix inversion (`ETKInvert`), Bessel functions for cable
//! line constants, and the statistics helpers used by loadshapes and Monte
//! Carlo modes.

use num_complex::Complex64;

use super::cmatrix::CMatrix;

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

    /// Upstream-compatible variant (`SetAMatrix_official` + `Invert`), used
    /// when the engine compat flag asks for official OpenDSS numerics.
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

/// In-place inversion of a real square matrix in column-major order, the same
/// pivoting algorithm as [`CMatrix::invert`] (Pascal `ETKInvert`).
/// Pascal error code 2 (singular) maps to `Err`.
pub fn etk_invert(a: &mut [f64], norder: usize) -> Result<(), super::cmatrix::SingularMatrix> {
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
            return Err(super::cmatrix::SingularMatrix);
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
/// For a single point both results equal that point, like the original.
pub fn mean_and_std_dev(data: &[f64]) -> (f64, f64) {
    if data.len() == 1 {
        return (data[0], data[0]);
    }
    let n = data.len() as f64;
    let mean = data.iter().sum::<f64>() / n;
    let s: f64 = data.iter().map(|&d| (mean - d) * (mean - d)).sum();
    (mean, (s / (n - 1.0)).sqrt())
}

/// Trapezoid-integrated mean and standard deviation of a curve y(x)
/// (Pascal `CurveMeanAndStdDev`).
pub fn curve_mean_and_std_dev(y: &[f64], x: &[f64]) -> (f64, f64) {
    let n = y.len();
    debug_assert_eq!(x.len(), n);
    if n == 1 {
        return (y[0], y[0]);
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

#[cfg(test)]
mod tests {
    use super::*;

    const J: Complex64 = Complex64::new(0.0, 1.0);

    fn polar_deg(mag: f64, ang_deg: f64) -> Complex64 {
        let ang = ang_deg.to_radians();
        c(mag * ang.cos(), mag * ang.sin())
    }

    #[test]
    fn balanced_positive_sequence_maps_to_pure_v1() {
        // Va = 1∠0, Vb = 1∠-120, Vc = 1∠120 → V0 = 0, V1 = 1, V2 = 0.
        let t = SymComp::precise();
        let vph = [
            polar_deg(1.0, 0.0),
            polar_deg(1.0, -120.0),
            polar_deg(1.0, 120.0),
        ];
        let mut v012 = [Complex64::ZERO; 3];
        t.phase_to_sym(&vph, &mut v012);
        assert!(v012[0].norm() < 1e-15, "V0 = {}", v012[0]);
        assert!((v012[1] - c(1.0, 0.0)).norm() < 1e-15, "V1 = {}", v012[1]);
        assert!(v012[2].norm() < 1e-15, "V2 = {}", v012[2]);
    }

    #[test]
    fn sym_comp_round_trip_precise() {
        let t = SymComp::precise();
        let vph = [c(1.02, 0.05), c(-0.6, -0.85), c(-0.45, 0.9)];
        let mut v012 = [Complex64::ZERO; 3];
        let mut back = [Complex64::ZERO; 3];
        t.phase_to_sym(&vph, &mut v012);
        t.sym_to_phase(&v012, &mut back);
        for (a, b) in vph.iter().zip(&back) {
            assert!((a - b).norm() < 1e-12, "{a} vs {b}");
        }
    }

    #[test]
    fn sym_comp_round_trip_official() {
        // The official matrices use truncated constants but the inverse is the
        // numeric inversion of the same matrix, so the round trip still holds.
        let t = SymComp::official();
        let vph = [c(0.98, -0.1), c(-0.55, -0.82), c(-0.5, 0.88)];
        let mut v012 = [Complex64::ZERO; 3];
        let mut back = [Complex64::ZERO; 3];
        t.phase_to_sym(&vph, &mut v012);
        t.sym_to_phase(&v012, &mut back);
        for (a, b) in vph.iter().zip(&back) {
            assert!((a - b).norm() < 1e-9, "{a} vs {b}");
        }
    }

    #[test]
    fn zero_sequence_set_maps_to_pure_v0() {
        let t = SymComp::precise();
        let v = c(0.9, 0.3);
        let vph = [v, v, v];
        let mut v012 = [Complex64::ZERO; 3];
        t.phase_to_sym(&vph, &mut v012);
        assert!((v012[0] - v).norm() < 1e-15);
        assert!(v012[1].norm() < 1e-15);
        assert!(v012[2].norm() < 1e-15);
    }

    #[test]
    fn terminal_power_and_k_powers() {
        let v = [c(100.0, 0.0), c(0.0, 100.0)];
        let i = [c(2.0, -1.0), c(1.0, 1.0)];
        // S = Σ V conj(I) = 100(2+j) + 100j(1-j) = (200+100) + j(100+100)
        let s = terminal_power_in(&v, &i, 2);
        assert_eq!(s, c(300.0, 200.0));

        let mut kw = [Complex64::ZERO; 2];
        calc_k_powers(&mut kw, &v, &i, 2);
        assert_eq!(kw[0], c(0.2, 0.1));
        assert_eq!(kw[1], c(0.1, 0.1));
    }

    #[test]
    fn xr_ratio_clamps() {
        assert_eq!(get_xr(c(1.0, 3.0)), 3.0);
        assert_eq!(get_xr(c(0.0, 5.0)), 9999.0);
        assert_eq!(get_xr(c(1e-9, 1.0)), 9999.0); // > 9999 clamps
        assert_eq!(get_xr(c(1.0, -3.0)), -3.0);
    }

    #[test]
    fn parallel_impedances() {
        // Two equal impedances in parallel halve.
        let z = c(2.0, 4.0);
        assert!((parallel_z(z, z) - z / 2.0).norm() < 1e-15);
        // Exactly opposite impedances: Pascal returns zero.
        assert_eq!(parallel_z(z, -z), Complex64::ZERO);
    }

    #[test]
    fn etk_invert_matches_cmatrix_invert_on_real_matrix() {
        // Same algorithm, real vs complex: results must agree exactly.
        let vals = [4.0, 1.0, 2.0, 0.5, 3.0, 1.5, 1.0, 2.5, 5.0]; // column-major 3x3
        let mut real = vals;
        etk_invert(&mut real, 3).unwrap();

        let mut cm = CMatrix::new(3);
        for j in 0..3 {
            for i in 0..3 {
                cm.set(i, j, c(vals[j * 3 + i], 0.0));
            }
        }
        cm.invert().unwrap();
        for j in 0..3 {
            for i in 0..3 {
                assert_eq!(real[j * 3 + i], cm.get(i, j).re);
                assert_eq!(cm.get(i, j).im, 0.0);
            }
        }
    }

    #[test]
    fn etk_invert_singular() {
        let mut a = [1.0, 2.0, 2.0, 4.0]; // rank 1
        assert!(etk_invert(&mut a, 2).is_err());
    }

    #[test]
    fn gauss_with_constant_rng_is_deterministic() {
        // 12 × 0.5 = 6 → exactly the mean.
        assert_eq!(gauss(10.0, 2.0, || 0.5), 10.0);
        // 12 × 0.25 = 3 → mean - 3σ.
        assert_eq!(gauss(0.0, 1.0, || 0.25), -3.0);
        assert_eq!(quasi_log_normal(3.0, || 0.5), 3.0); // e^0 * mean
    }

    #[test]
    fn mean_and_std_dev_basics() {
        let (m, s) = mean_and_std_dev(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        assert!((m - 5.0).abs() < 1e-15);
        assert!((s - (32.0f64 / 7.0).sqrt()).abs() < 1e-12);
        // single point: Pascal quirk — stddev equals the value
        assert_eq!(mean_and_std_dev(&[3.5]), (3.5, 3.5));
    }

    #[test]
    fn curve_mean_of_constant_curve() {
        let x = [0.0, 1.0, 3.0, 6.0];
        let y = [2.0, 2.0, 2.0, 2.0];
        let (m, s) = curve_mean_and_std_dev(&y, &x);
        assert!((m - 2.0).abs() < 1e-15);
        assert!(s.abs() < 1e-15);

        // ramp 0→1 over [0,1]: trapezoid mean = 0.5
        let x = [0.0, 1.0];
        let y = [0.0, 1.0];
        let (m, _) = curve_mean_and_std_dev(&y, &x);
        assert!((m - 0.5).abs() < 1e-15);
    }

    #[test]
    fn bessel_small_argument_series_values() {
        // I0(x) ≈ 1 + x²/4 + x⁴/64, I1(x) ≈ x/2 + x³/16 for small real x.
        let x = 0.1;
        let i0 = bessel_i0(c(x, 0.0));
        let expect = 1.0 + x * x / 4.0 + x.powi(4) / 64.0 + x.powi(6) / 2304.0;
        assert!((i0.re - expect).abs() < 1e-12, "{} vs {expect}", i0.re);
        assert!(i0.im == 0.0);

        let i1 = bessel_i1(c(x, 0.0));
        let expect = x / 2.0 + x.powi(3) / 16.0 + x.powi(5) / 384.0 + x.powi(7) / 18432.0;
        assert!((i1.re - expect).abs() < 1e-15, "{} vs {expect}", i1.re);
    }

    #[test]
    fn bessel_complex_argument_converges() {
        // I0(j x) = J0(x); J0(1) = 0.7651976865579666
        let i0 = bessel_i0(J * 1.0);
        assert!((i0.re - 0.7651976865579666).abs() < 1e-12, "{}", i0.re);
        assert!(i0.im.abs() < 1e-15);
    }

    #[test]
    fn nema_unbalance() {
        let balanced = [
            polar_deg(1.0, 0.0),
            polar_deg(1.0, -120.0),
            polar_deg(1.0, 120.0),
        ];
        assert!(pct_nema_unbalance(&balanced) < 1e-12);

        let unbalanced = [
            polar_deg(1.1, 0.0),
            polar_deg(1.0, -120.0),
            polar_deg(0.9, 120.0),
        ];
        // avg = 1.0, max diff = 0.1 → 10%
        assert!((pct_nema_unbalance(&unbalanced) - 10.0).abs() < 1e-12);

        assert_eq!(pct_nema_unbalance(&[Complex64::ZERO; 3]), 0.0);
    }

    #[test]
    fn pi_ctrl_step_response() {
        // Reproduce the difference equation by hand for a few steps.
        let mut ctrl = PiCtrl::new();
        let sp = 1.0;
        // step1: num=[0, 0.02], den=[0, 0]
        assert_eq!(ctrl.solve_pi(sp), 0.0);
        // step2: num=[0.02, 0.02], den=[0, 0.02*0.8647]
        let d2 = ctrl.solve_pi(sp);
        assert!((d2 - 0.02 * 0.8647).abs() < 1e-15);
        // step3: den = num0*kNum + den0*kDen
        let d3 = ctrl.solve_pi(sp);
        assert!((d3 - (0.02 * 0.8647 + d2 * 0.1353)).abs() < 1e-15);
    }
}
