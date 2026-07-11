//! Unit tests for the Carson engine, pinned against the dss-python oracle
//! (PIN.txt 0.15.7 / backend 0.14.5). Reference matrices were probed by
//! building the same geometry through a `Line` and reading `Rmatrix`/`Xmatrix`
//! (ohm per unit length, here ohm/m) and `Cmatrix` (nF per unit length, here
//! nF/m) — i.e. the engine outputs that the geometry path produces. See the
//! WP7.1 probe in STATUS.md.

use super::*;

const M: i32 = 4; // LineUnits::Meter code
const KM: i32 = 3; // LineUnits::Km code

// Shared SI wire: rac = 3e-4 ohm/m, gmr = 0.005 m, radius = 0.01 m.
// The oracle derives Rdc = Rac / 1.02 when Rdc is unset (ConductorData side
// effect), which the DERI skin-effect term consumes.
const RAC: f64 = 3.0e-4;
const GMR: f64 = 0.005;
const RADIUS: f64 = 0.01;

/// Build a LineConstants for an overhead geometry of `(x, h)` meter coordinates,
/// mirroring the assignment order of `TLineGeometryObj.UpdateLineGeometryData`.
fn build(coords: &[(f64, f64)]) -> LineConstants {
    let n = coords.len();
    let mut lc = LineConstants::new(n);
    for (i, &(x, h)) in coords.iter().enumerate() {
        lc.set_x(i, M, x);
        lc.set_y(i, M, h);
        lc.set_radius(i, M, RADIUS);
        lc.set_capradius(i, M, RADIUS);
        lc.set_gmr(i, M, GMR);
        lc.set_rdc(i, M, RAC / 1.02);
        lc.set_rac(i, M, RAC);
    }
    lc
}

// Shared cable geometry: 3 buried cables (h irrelevant for cable Calc) at
// x = 0/0.1/0.2 m. Same core conductor for CN and TS.
const CABLE_COORDS3: [(f64, f64); 3] = [(0.0, -1.2), (0.1, -1.2), (0.2, -1.2)];

/// Common core-conductor + insulation setters shared by `build_cn`/`build_ts`,
/// mirroring `UpdateLineGeometryData`'s assignment order.
fn set_cable_common(lc: &mut LineConstants) {
    for (i, &(x, h)) in CABLE_COORDS3.iter().enumerate() {
        lc.set_x(i, M, x);
        lc.set_y(i, M, h);
        lc.set_radius(i, M, 0.005);
        lc.set_capradius(i, M, 0.005);
        lc.set_gmr(i, M, 0.004);
        lc.set_rdc(i, M, 1.0e-4);
        lc.set_rac(i, M, 1.05e-4);
        lc.set_eps_r(i, 2.3);
        lc.set_ins_layer(i, M, 0.004);
        lc.set_dia_ins(i, M, 0.022);
        lc.set_dia_cable(i, M, 0.030);
    }
}

fn build_cn() -> LineConstants {
    let mut lc = LineConstants::new_cn(3);
    set_cable_common(&mut lc);
    for i in 0..3 {
        lc.set_k_strand(i, 16);
        lc.set_dia_strand(i, M, 0.001);
        lc.set_gmr_strand(i, M, 0.0004);
        lc.set_r_strand(i, M, 2.0e-3);
    }
    lc
}

fn build_ts() -> LineConstants {
    let mut lc = LineConstants::new_ts(3);
    set_cable_common(&mut lc);
    for i in 0..3 {
        lc.set_dia_shield(i, M, 0.025);
        lc.set_tape_layer(i, M, 0.0002);
        lc.set_tape_lap(i, 20.0);
    }
    lc
}

/// Build a 4-conductor CN cable (3 phases + a 4th bare-neutral core at x = 0.3),
/// mirroring `cn_reduce()` in the WP7.1 probe. The caller sets nphases = 3 so the
/// CN self-Z/capacitance loops run over phases only and the 4th core acts as a
/// bare neutral that gets Kron-reduced out.
fn build_cn4() -> LineConstants {
    let mut lc = LineConstants::new_cn(4);
    let coords = [(0.0, -1.2), (0.1, -1.2), (0.2, -1.2), (0.3, -1.2)];
    for (i, &(x, h)) in coords.iter().enumerate() {
        lc.set_x(i, M, x);
        lc.set_y(i, M, h);
        lc.set_radius(i, M, 0.005);
        lc.set_capradius(i, M, 0.005);
        lc.set_gmr(i, M, 0.004);
        lc.set_rdc(i, M, 1.0e-4);
        lc.set_rac(i, M, 1.05e-4);
        lc.set_eps_r(i, 2.3);
        lc.set_ins_layer(i, M, 0.004);
        lc.set_dia_ins(i, M, 0.022);
        lc.set_dia_cable(i, M, 0.030);
        lc.set_k_strand(i, 16);
        lc.set_dia_strand(i, M, 0.001);
        lc.set_gmr_strand(i, M, 0.0004);
        lc.set_r_strand(i, M, 2.0e-3);
    }
    lc
}

// Coaxial insulation capacitance is the same for CN and TS here (identical
// EpsR/DiaIns/InsLayer); off-diagonals are exactly zero for shielded cables.
const CABLE_C3_NF: [f64; 9] = [
    2.830890564838e-01,
    0.0,
    0.0,
    0.0,
    2.830890564838e-01,
    0.0,
    0.0,
    0.0,
    2.830890564838e-01,
];

fn assert_close(got: f64, want: f64, what: &str) {
    let tol = 1e-8 * want.abs().max(1e-12);
    assert!(
        (got - want).abs() <= tol,
        "{what}: got {got:.12e}, want {want:.12e} (|d|={:.3e})",
        (got - want).abs()
    );
}

/// Compare the engine's base Z (ohm/m) against an oracle reference, row-major.
fn check_z(lc: &LineConstants, z_ref: &[(f64, f64)]) {
    let n = lc.num_conductors();
    let z = lc.z_base();
    for i in 0..n {
        for j in 0..n {
            let (re, im) = z_ref[i * n + j];
            let g = z.get(i, j);
            assert_close(g.re, re, &format!("Z[{i}][{j}].re"));
            assert_close(g.im, im, &format!("Z[{i}][{j}].im"));
        }
    }
}

/// Compare the engine's base capacitance C = Im(Yc)/w (F/m) against the oracle
/// `Cmatrix` (nF/m), row-major.
fn check_c(lc: &LineConstants, c_ref_nf: &[f64]) {
    let n = lc.num_conductors();
    let yc = lc.yc_base();
    let w = lc.omega();
    for i in 0..n {
        for j in 0..n {
            let want = c_ref_nf[i * n + j] * 1e-9; // nF/m -> F/m
            let got = yc.get(i, j).im / w;
            assert_close(got, want, &format!("C[{i}][{j}]"));
        }
    }
}

// 3-phase overhead, conductors at x = 0/1/2 m, h = 10 m.
const COORDS3: [(f64, f64); 3] = [(0.0, 10.0), (1.0, 10.0), (2.0, 10.0)];

const C3_NF: [f64; 9] = [
    8.941431489720e-03,
    -2.907193315935e-03,
    -1.568246629107e-03,
    -2.907193315935e-03,
    9.611612264845e-03,
    -2.907193315935e-03,
    -1.568246629107e-03,
    -2.907193315935e-03,
    8.941431489720e-03,
];

#[test]
fn deri_full_3cond() {
    let mut lc = build(&COORDS3);
    lc.calc(60.0, DERI);
    let z_ref = [
        (3.525947626277e-04, 9.150978496084e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (5.807470316267e-05, 4.633520928126e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (3.525947626277e-04, 9.150978496084e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (5.807470316267e-05, 4.633520928126e-04),
        (5.807483299046e-05, 5.156141524879e-04),
        (3.525947626277e-04, 9.150978496084e-04),
    ];
    check_z(&lc, &z_ref);
    check_c(&lc, &C3_NF);
}

/// `cabs_fpc`/`csqrt_fpc`/`cln_fpc` reproduce the FPC `ucomplex`
/// `cmod`/`csqrt`/`cln` bit-for-bit: the naive `sqrt(re²+im²)` modulus (not
/// `hypot`), the Numerical-Recipes stable `csqrt` (not the polar `from_polar`
/// form), and `ln(cmod)+j·atan2`. The Carson DERI and cable earth terms call
/// these; `num_complex`'s `.sqrt()`/`.ln()` round the last bit differently,
/// which surfaced as a 1-ULP gap in the earth-return resistance part — the same
/// class of bug as the `cdiv_fpc` division mismatch. Bits captured from the
/// x86_64 FPC `ucomplex` RTL.
#[test]
fn fpc_complex_primitives_match_ucomplex_not_num_complex() {
    // General sqrt: FPC algebraic (NR) vs num_complex polar — 1 ULP apart.
    let z = Complex64::new(1.5, -2.25);
    let s = csqrt_fpc(z);
    assert_eq!(s.re.to_bits(), 0x3FF7329BF464ACB7);
    assert_eq!(s.im.to_bits(), 0xBFE8D47E8FC83CA6);
    assert_ne!(z.sqrt().re.to_bits(), s.re.to_bits());

    // Negative-real branch (the other sign split).
    let z2 = Complex64::new(-5.5, 2.25);
    let s2 = csqrt_fpc(z2);
    assert_eq!(s2.re.to_bits(), 0x3FDE19FCBCC29600);
    assert_eq!(s2.im.to_bits(), 0x4003229FCE84B2B2);
    assert_ne!(z2.sqrt().re.to_bits(), s2.re.to_bits());

    // A real DERI earth-term `Csqrt(hterm²+xterm²)` argument — bit-exact to RTL.
    let arg = Complex64::new(
        f64::from_bits(0x40D9C5B937764D60),
        f64::from_bits(0xC12A8F7A43C125DD),
    );
    let la = csqrt_fpc(arg);
    assert_eq!(la.re.to_bits(), 0x4084EDFB5C679BD3);
    assert_eq!(la.im.to_bits(), 0xC0844DF9CF27F050);

    // cln = ln(cabs) + j·atan2; cabs is the naive sqrt(re²+im²).
    let l = cln_fpc(Complex64::new(3.0, 4.0));
    assert_eq!(l.re.to_bits(), 0x3FF9C041F7ED8D33);
    assert_eq!(l.im.to_bits(), 0x3FEDAC670561BB4F);
    assert_eq!(
        cabs_fpc(Complex64::new(1.5, -2.25)).to_bits(),
        0x4005A22073490377
    );
}

/// DERI overhead per-meter `Z` is **bit-for-bit** to the oracle wherever the
/// remaining libm floor doesn't bite: the diagonal (Bessel skin-effect `Zint` +
/// earth `Ze`, via `csqrt_fpc`/`cln_fpc`/`cdiv_fpc`) and the distance-2 mutual.
/// The adjacent mutual's *real* part keeps a proven 1-ULP `arctan2` (`carg`)
/// floor — `Fme`/`Cinv`/`hterm`/`Csqrt`/`ln(cmod)` are all bit-exact, only the
/// final `arctan2` in `Cln`'s imag differs (the same external-libm last-bit
/// class as faer-vs-KLU). Guards the get_ze/get_zint wiring of the FPC
/// primitives. Bits from the x86_64 oracle (`Lines.Rmatrix`/`Xmatrix`, 1 m).
#[test]
fn deri_overhead_z_bit_exact_except_arctan2_floor() {
    let mut lc = build(&COORDS3);
    lc.calc(60.0, DERI);
    let z = lc.z_base();
    for i in 0..3 {
        assert_eq!(z.get(i, i).re.to_bits(), 0x3f371b8ef966ede3, "Zdiag.re");
        assert_eq!(z.get(i, i).im.to_bits(), 0x3f4dfc65ab19412d, "Zdiag.im");
    }
    assert_eq!(z.get(0, 2).re.to_bits(), 0x3f0e72a79b4186fe, "Z02.re");
    assert_eq!(z.get(0, 2).im.to_bits(), 0x3f3e5dc215cd4128, "Z02.im");
    // Adjacent mutual: imag bit-exact, real within the 1-ULP arctan2 floor.
    assert_eq!(z.get(0, 1).im.to_bits(), 0x3f40e548f60d9a32, "Z01.im");
    let want01re = 0x3f0e72ac113bb68e_i64;
    assert!(
        (z.get(0, 1).re.to_bits() as i64 - want01re).abs() <= 1,
        "Z01.re within 1 ULP of oracle (arctan2 floor), got {:016x}",
        z.get(0, 1).re.to_bits()
    );
}

#[test]
fn simple_carson_full_3cond() {
    let mut lc = build(&COORDS3);
    lc.calc(60.0, SIMPLE_CARSON);
    let z_ref = [
        (3.592176235097e-04, 9.080731425744e-04),
        (5.921762350974e-05, 5.085894441415e-04),
        (5.921762350974e-05, 4.563273805293e-04),
        (5.921762350974e-05, 5.085894441415e-04),
        (3.592176235097e-04, 9.080731425744e-04),
        (5.921762350974e-05, 5.085894441415e-04),
        (5.921762350974e-05, 4.563273805293e-04),
        (5.921762350974e-05, 5.085894441415e-04),
        (3.592176235097e-04, 9.080731425744e-04),
    ];
    check_z(&lc, &z_ref);
    check_c(&lc, &C3_NF); // capacitance is earth-model independent
}

#[test]
fn full_carson_full_3cond() {
    let mut lc = build(&COORDS3);
    lc.calc(60.0, FULL_CARSON);
    let z_ref = [
        (3.577509712142e-04, 9.096497377052e-04),
        (5.775083581973e-05, 5.101660729647e-04),
        (5.775042974772e-05, 4.579041104292e-04),
        (5.775083581973e-05, 5.101660729647e-04),
        (3.577509712142e-04, 9.096497377052e-04),
        (5.775083581973e-05, 5.101660729647e-04),
        (5.775042974772e-05, 4.579041104292e-04),
        (5.775083581973e-05, 5.101660729647e-04),
        (3.577509712142e-04, 9.096497377052e-04),
    ];
    check_z(&lc, &z_ref);
    check_c(&lc, &C3_NF);
}

#[test]
fn deri_reduce_4cond_to_3() {
    // 3 phases + 1 neutral at (1, 12); reduce out the neutral (Kron).
    let coords = [(0.0, 10.0), (1.0, 10.0), (2.0, 10.0), (1.0, 12.0)];
    let mut lc = build(&coords);
    lc.set_nphases(3);
    lc.calc(60.0, DERI);
    lc.reduce();

    // After Reduce, the public z_matrix/yc_matrix use the reduced 3x3.
    let z = lc.z_matrix(60.0, 1.0, M, DERI);
    let yc = lc.yc_matrix(1.0, M);
    let w = lc.omega();

    let z_ref = [
        (3.770208623296e-04, 7.019407348531e-04),
        (8.343915794981e-05, 2.986360481570e-04),
        (8.250080286461e-05, 2.501949780572e-04),
        (8.343915794981e-05, 2.986360481570e-04),
        (3.789232335261e-04, 6.942314211641e-04),
        (8.343915794981e-05, 2.986360481570e-04),
        (8.250080286461e-05, 2.501949780572e-04),
        (8.343915794981e-05, 2.986360481570e-04),
        (3.770208623296e-04, 7.019407348531e-04),
    ];
    let c_ref_nf = [
        9.210099265515e-03,
        -2.642486857248e-03,
        -1.299578853311e-03,
        -2.642486857248e-03,
        9.872415813254e-03,
        -2.642486857248e-03,
        -1.299578853311e-03,
        -2.642486857248e-03,
        9.210099265515e-03,
    ];
    for i in 0..3 {
        for j in 0..3 {
            let (re, im) = z_ref[i * 3 + j];
            assert_close(z.get(i, j).re, re, &format!("Zr[{i}][{j}].re"));
            assert_close(z.get(i, j).im, im, &format!("Zr[{i}][{j}].im"));
            assert_close(
                yc.get(i, j).im / w,
                c_ref_nf[i * 3 + j] * 1e-9,
                &format!("Cr[{i}][{j}]"),
            );
        }
    }
}

#[test]
fn conductors_in_same_space_detects_overlap() {
    let mut lc = LineConstants::new(2);
    lc.set_radius(0, M, 0.5);
    lc.set_radius(1, M, 0.5);
    lc.set_x(0, M, 0.0);
    lc.set_y(0, M, 10.0);
    lc.set_x(1, M, 0.2);
    lc.set_y(1, M, 10.0);
    assert!(lc.conductors_in_same_space().is_some());

    // a zero-height conductor is also flagged
    let mut lc2 = LineConstants::new(1);
    lc2.set_radius(0, M, 0.01);
    lc2.set_y(0, M, 0.0);
    assert!(lc2.conductors_in_same_space().is_some());
}

#[test]
fn cn_cable_deri_3cond() {
    let mut lc = build_cn();
    assert_eq!(lc.kind(), LineConstantsKind::ConcentricNeutral);
    lc.calc(60.0, DERI);
    let z_ref = [
        (1.957766526264e-04, 1.402105660670e-04),
        (2.071515058850e-05, -1.736794561581e-05),
        (7.705339488750e-06, -1.452216497679e-05),
        (2.071515058850e-05, -1.736794561581e-05),
        (1.844408315520e-04, 1.418811316531e-04),
        (2.071515058850e-05, -1.736794561581e-05),
        (7.705339488750e-06, -1.452216497679e-05),
        (2.071515058850e-05, -1.736794561581e-05),
        (1.957766526264e-04, 1.402105660670e-04),
    ];
    check_z(&lc, &z_ref);
    check_c(&lc, &CABLE_C3_NF);
}

#[test]
fn ts_cable_deri_3cond() {
    let mut lc = build_ts();
    assert_eq!(lc.kind(), LineConstantsKind::TapeShield);
    lc.calc(60.0, DERI);
    let z_ref = [
        (4.675825330004e-04, 4.983304721750e-04),
        (3.583518060982e-04, 2.468927740652e-04),
        (3.436688380811e-04, 2.058672035803e-04),
        (3.583518060982e-04, 2.468927740652e-04),
        (4.783159246306e-04, 4.782357442920e-04),
        (3.583518060982e-04, 2.468927740652e-04),
        (3.436688380811e-04, 2.058672035803e-04),
        (3.583518060982e-04, 2.468927740652e-04),
        (4.675825330004e-04, 4.983304721750e-04),
    ];
    check_z(&lc, &z_ref);
    check_c(&lc, &CABLE_C3_NF);
}

/// CN cable under the simple-Carson earth model (code 1) — the cable `Calc`
/// reaches `get_zint`/`get_ze` with a different earth branch than DERI. Only the
/// earth-return term changes; capacitance is earth-model independent.
#[test]
fn cn_cable_carson_3cond() {
    let mut lc = build_cn();
    lc.calc(60.0, SIMPLE_CARSON);
    let z_ref = [
        (1.995928701217e-04, 1.402304548966e-04),
        (2.071020560082e-05, -1.735033610268e-05),
        (7.694803927944e-06, -1.450227660167e-05),
        (2.071020560082e-05, -1.735033610268e-05),
        (1.882666874078e-04, 1.418959689162e-04),
        (2.071020560082e-05, -1.735033610268e-05),
        (7.694803927944e-06, -1.450227660167e-05),
        (2.071020560082e-05, -1.735033610268e-05),
        (1.995928701217e-04, 1.402304548966e-04),
    ];
    check_z(&lc, &z_ref);
    check_c(&lc, &CABLE_C3_NF);
}

/// CN cable under the full-Carson earth model (code 2).
#[test]
fn cn_cable_fullcarson_3cond() {
    let mut lc = build_cn();
    lc.calc(60.0, FULL_CARSON);
    let z_ref = [
        (1.995938779693e-04, 1.402299934127e-04),
        (2.071092731833e-05, -1.735087539082e-05),
        (7.695813121396e-06, -1.450273589516e-05),
        (2.071092731833e-05, -1.735087539082e-05),
        (1.882671677572e-04, 1.418954163443e-04),
        (2.071092731833e-05, -1.735087539082e-05),
        (7.695813121396e-06, -1.450273589516e-05),
        (2.071092731833e-05, -1.735087539082e-05),
        (1.995938779693e-04, 1.402299934127e-04),
    ];
    check_z(&lc, &z_ref);
    check_c(&lc, &CABLE_C3_NF);
}

/// TS cable under the simple-Carson earth model (code 1).
#[test]
fn ts_cable_carson_3cond() {
    let mut lc = build_ts();
    lc.calc(60.0, SIMPLE_CARSON);
    let z_ref = [
        (4.689993879058e-04, 4.983332077809e-04),
        (3.559473809878e-04, 2.469610035063e-04),
        (3.412589418393e-04, 2.058699387985e-04),
        (3.559473809878e-04, 2.469610035063e-04),
        (4.797454922198e-04, 4.783691723432e-04),
        (3.559473809878e-04, 2.469610035063e-04),
        (3.412589418393e-04, 2.058699387985e-04),
        (3.559473809878e-04, 2.469610035063e-04),
        (4.689993879058e-04, 4.983332077809e-04),
    ];
    check_z(&lc, &z_ref);
    check_c(&lc, &CABLE_C3_NF);
}

/// TS cable under the full-Carson earth model (code 2).
#[test]
fn ts_cable_fullcarson_3cond() {
    let mut lc = build_ts();
    lc.calc(60.0, FULL_CARSON);
    let z_ref = [
        (4.690930328382e-04, 4.984054507193e-04),
        (3.560427762456e-04, 2.470305399146e-04),
        (3.413525782443e-04, 2.059421850446e-04),
        (3.560427762456e-04, 2.470305399146e-04),
        (4.798425644940e-04, 4.784359589267e-04),
        (3.560427762456e-04, 2.470305399146e-04),
        (3.413525782443e-04, 2.059421850446e-04),
        (3.560427762456e-04, 2.470305399146e-04),
        (4.690930328382e-04, 4.984054507193e-04),
    ];
    check_z(&lc, &z_ref);
    check_c(&lc, &CABLE_C3_NF);
}

/// Above the power-frequency band (f ≥ 1 kHz) the cable `Calc` uses the actual
/// radius for the core self spacing and keeps the conductor internal reactance
/// (`Zi.im`) — the branch the 60 Hz cable tests never exercise. Asserts Z for
/// both CN and TS at 5 kHz. Capacitance is omitted: the oracle's `Cmatrix`
/// getter scales reported nF by the solve frequency (it has no real f branch).
#[test]
fn cable_high_freq_radius_branch() {
    let mut cn = build_cn();
    cn.calc(5000.0, DERI);
    let cn_ref = [
        (5.437160837897e-04, 7.401681588699e-03),
        (2.261818700187e-06, 2.490864286186e-06),
        (9.535849721604e-07, 9.911880118441e-07),
        (2.261818700187e-06, 2.490864286186e-06),
        (5.426039985115e-04, 7.400452766219e-03),
        (2.261818700187e-06, 2.490864286188e-06),
        (9.535849721604e-07, 9.911880118441e-07),
        (2.261818700187e-06, 2.490864286188e-06),
        (5.437160837897e-04, 7.401681588699e-03),
    ];
    check_z(&cn, &cn_ref);

    let mut ts = build_ts();
    ts.calc(5000.0, DERI);
    let ts_ref = [
        (2.311149680199e-03, 6.266344148098e-03),
        (1.602361294848e-05, -9.423065093916e-05),
        (1.329151923265e-06, -4.171522833319e-05),
        (1.602361294848e-05, -9.423065093916e-05),
        (2.299070719031e-03, 6.309726214916e-03),
        (1.602361294848e-05, -9.423065093916e-05),
        (1.329151923265e-06, -4.171522833319e-05),
        (1.602361294848e-05, -9.423065093916e-05),
        (2.311149680199e-03, 6.266344148098e-03),
    ];
    check_z(&ts, &ts_ref);
}

/// CN cable, 4 conductors (3 phases + a bare-neutral core) Kron-reduced to 3 —
/// exercises the cable reduced-Z/Yc path (`fz_reduced`/`fyc_reduced`) and the
/// `reduced_size > 0` re-reduce branch in `calc_cn`. The asymmetric diagonal is
/// expected: the neutral at x = 0.3 sits closer to phase 3 than to phase 1.
#[test]
fn cn_cable_reduce_4cond_to_3() {
    let mut lc = build_cn4();
    lc.set_nphases(3);
    lc.calc(60.0, DERI);
    lc.reduce();

    let z = lc.z_matrix(60.0, 1.0, M, DERI);
    let yc = lc.yc_matrix(1.0, M);
    let w = lc.omega();

    let z_ref = [
        (1.958129515493e-04, 1.416226769335e-04),
        (2.075131382841e-05, -1.559146688412e-05),
        (5.884840587231e-06, -1.118926130678e-05),
        (2.075131382841e-05, -1.559146688412e-05),
        (1.844743739553e-04, 1.441159324270e-04),
        (1.840280999639e-05, -1.318786861888e-05),
        (5.884840587231e-06, -1.118926130678e-05),
        (1.840280999639e-05, -1.318786861888e-05),
        (1.870469567699e-04, 1.455055837909e-04),
    ];
    for i in 0..3 {
        for j in 0..3 {
            let (re, im) = z_ref[i * 3 + j];
            assert_close(z.get(i, j).re, re, &format!("Zr[{i}][{j}].re"));
            assert_close(z.get(i, j).im, im, &format!("Zr[{i}][{j}].im"));
            assert_close(
                yc.get(i, j).im / w,
                CABLE_C3_NF[i * 3 + j] * 1e-9,
                &format!("Cr[{i}][{j}]"),
            );
        }
    }
}

/// Cable `ConductorsInSameSpace` uses `0.5*DiaCable` for neutral conductors and
/// drops the height check — a buried (negative-height) cable is *not* flagged,
/// unlike the overhead validator.
#[test]
fn cable_same_space_uses_diacable_and_ignores_height() {
    let lc = build_cn();
    assert!(lc.conductors_in_same_space().is_none()); // valid buried cables

    // Two neutral conductors (nphases = 0) 0.02 m apart. Their effective radius
    // is 0.5*DiaCable = 0.015 m each, so 0.02 < 0.03 ⇒ overlap. (As phases they
    // would use the 0.005 m core radius and would not overlap — this exercises
    // the DiaCable branch.)
    let mut lc2 = LineConstants::new_cn(2);
    lc2.set_nphases(0);
    for i in 0..2 {
        lc2.set_dia_cable(i, M, 0.030);
        lc2.set_y(i, M, -1.0);
    }
    lc2.set_x(0, M, 0.0);
    lc2.set_x(1, M, 0.02);
    let msg = lc2.conductors_in_same_space();
    assert!(msg.is_some());
    assert!(msg.unwrap().starts_with("Cable conductors 1 and 2"));
}

/// Above the power-frequency band (f ≥ 1 kHz) `Calc` uses the actual radius for
/// the self spacing and keeps the conductor internal reactance (`Zi.im`) — the
/// branch the 60 Hz tests never exercise.
#[test]
fn overhead_high_freq_radius_branch() {
    let mut lc = build(&COORDS3);
    lc.calc(5000.0, DERI);
    let z_ref = [
        (4.923757922619e-03, 5.945699655942e-02),
        (4.164436672700e-03, 2.984975434922e-02),
        (4.163753358720e-03, 2.549475346052e-02),
        (4.164436672700e-03, 2.984975434922e-02),
        (4.923757922619e-03, 5.945699655942e-02),
        (4.164436672700e-03, 2.984975434922e-02),
        (4.163753358720e-03, 2.549475346052e-02),
        (4.164436672700e-03, 2.984975434922e-02),
        (4.923757922619e-03, 5.945699655942e-02),
    ];
    check_z(&lc, &z_ref);
    // Capacitance has no frequency branch; the oracle's Cmatrix getter scales
    // its reported nF by the solve frequency, so C is asserted only in the
    // power-frequency tests above.
}

/// Changing `rho_earth` after a `Calc` sets `frho_changed`, and the next
/// `z_matrix` recomputes against the new earth resistivity (oracle: rho = 200).
#[test]
fn rho_earth_change_forces_recalc() {
    let mut lc = build(&COORDS3);
    lc.calc(60.0, DERI); // rho = 100 (default)
    lc.set_rho_earth(200.0); // frho_changed = true; recompute Fme (Frequency ≥ 0)
    let z = lc.z_matrix(60.0, 1.0, M, DERI); // same f, but rho changed ⇒ recalc

    let z_ref = [
        (3.529258314601e-04, 9.408891165221e-04),
        (5.840592326001e-05, 5.414054185596e-04),
        (5.840585774351e-05, 4.891433563583e-04),
        (5.840592326001e-05, 5.414054185596e-04),
        (3.529258314601e-04, 9.408891165221e-04),
        (5.840592326001e-05, 5.414054185596e-04),
        (5.840585774351e-05, 4.891433563583e-04),
        (5.840592326001e-05, 5.414054185596e-04),
        (3.529258314601e-04, 9.408891165221e-04),
    ];
    for i in 0..3 {
        for j in 0..3 {
            let (re, im) = z_ref[i * 3 + j];
            assert_close(z.get(i, j).re, re, &format!("Zrho[{i}][{j}].re"));
            assert_close(z.get(i, j).im, im, &format!("Zrho[{i}][{j}].im"));
        }
    }
}

/// `z_matrix`/`yc_matrix` scale the per-meter base by `from_per_meter(units) *
/// length`. Per-km over 2 km ⇒ factor 2000 vs the per-meter base.
#[test]
fn matrix_unit_and_length_conversion() {
    let mut lc = build(&COORDS3);
    lc.calc(60.0, DERI);
    let base_z = lc.z_base().clone();
    let base_yc = lc.yc_base().clone();

    let z = lc.z_matrix(60.0, 2.0, KM, DERI); // 2 km, ohm/km
    let yc = lc.yc_matrix(2.0, KM);
    let factor = 1000.0 * 2.0; // from_per_meter(km) * length

    for i in 0..3 {
        for j in 0..3 {
            assert_close(
                z.get(i, j).re,
                base_z.get(i, j).re * factor,
                &format!("Zconv[{i}][{j}].re"),
            );
            assert_close(
                z.get(i, j).im,
                base_z.get(i, j).im * factor,
                &format!("Zconv[{i}][{j}].im"),
            );
            assert_close(
                yc.get(i, j).im,
                base_yc.get(i, j).im * factor,
                &format!("Ycconv[{i}][{j}].im"),
            );
        }
    }
}
