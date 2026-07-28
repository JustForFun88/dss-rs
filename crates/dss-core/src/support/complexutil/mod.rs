//! Polar/angle helpers, port of `Shared/DSSUcomplex.pas`.
//!
//! Most of that unit maps directly onto `num_complex` (`cmplx` → `Complex64::new`,
//! `cabs` → `norm`, `cabs2` → `norm_sqr`, `cong` → `conj`, `cZERO`/`cONE` →
//! `Complex64::ZERO`/`ONE`) and is not re-wrapped. What lives here are the
//! polar-form conversions, which deliberately reproduce the original's
//! truncated constants: angles in degrees are scaled by `57.29577951`, and
//! the quadrant correction in `cang` adds the truncated `3.14159265359` —
//! both visible in engine report output.

#[cfg(test)]
mod tests;

use num_complex::Complex64;

/// Pascal `polar` record: magnitude and angle (radians or degrees depending
/// on which conversion produced it).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Polar {
    pub mag: f64,
    pub ang: f64,
}

/// Scalar to polar (Pascal `topolar`).
pub fn to_polar(mag: f64, ang: f64) -> Polar {
    Polar { mag, ang }
}

// TODO(compat): truncated constants reproduced from DSSUcomplex.pas. The clean
// fix is `f64::consts::PI` and `180/PI` — named by upstream itself, which
// carries `// TODO: better precision` on both `CDANG` (`:87`) and
// `PDEGtoCompLeX` (`:120`) and `// TODO: remove for 0.13` on the local `PI`
// (`:60`); r4133 `Version8/Source/Shared/Ucomplex.pas:98/120/251` is identical,
// so both gating oracles carry them.
//
// **Stage F.3z measured the row and found it is not a pair of independent
// constants but one round trip, which is why it escapes.**
//
// * `TRUNCATED_RAD_TO_DEG` is 5.38e-11 relative and `TRUNCATED_PI` 6.6e-14 —
//   both far below every calibrated floor, so the row *looks* free.
// * It is not, because [`cdang`] and [`pdeg_to_complex`] are **inverses of each
//   other and the engine round-trips through them**: a Spectrum harmonic (or a
//   source phase) is typed in degrees, converted to a phasor by the divisor, and
//   reported back in degrees by the multiplier. The two truncations cancel
//   exactly. Flipping only the reporting half is therefore *incoherent* rather
//   than conservative: it moves 0 corpus cases but breaks the identity, printing
//   `30.0000000016139` where the oracle (and the truncated engine) print `30`
//   for `dump_spectrum`, and shifting `dump3_debug`'s `-179.999999980645` by
//   1.19e-11 deg.
// * Flipping **both** — the only coherent form — makes the engine strictly more
//   accurate (residues shrink: `dump_spectrum`'s `0.1·√3` becomes exactly
//   `0.173205080756888`, `export_seqvoltages`' `V0` residual falls from
//   4.31951e-6 to 1.49998e-11) and *that* is what costs: **16 of the 520** gated
//   corpus cases, 5 `wasm_*` r4133 goldens (worst `V[SOURCEBUS.1].im`
//   |rel| 9.95e-7 against an allowed 1.027e-6), and 7 further byte goldens.
//
// So the row is a whole-artifact re-baseline against a re-captured oracle — an
// UPGRADE rung, which the parity lane may never run — not a Stage F lane flip.
#[allow(clippy::approx_constant)]
const TRUNCATED_PI: f64 = 3.14159265359;
const TRUNCATED_RAD_TO_DEG: f64 = 57.29577951;

/// The private `ATAN2(x, iy)` from DSSUcomplex.pas: a hand-rolled atan2 whose
/// quadrant correction uses the truncated pi. Argument order is (re, im).
///
/// TODO(compat): replace with `f64::atan2` once the truncated-constant pair
/// above is re-baselined — this function is the *consumer* of `TRUNCATED_PI`,
/// so it moves with that row and cannot be flipped on its own (see the
/// F.3z measurement at the constants).
fn pascal_atan2(x: f64, iy: f64) -> f64 {
    if x < 0.0 && iy >= 0.0 {
        (iy / x).atan() + TRUNCATED_PI
    } else if x < 0.0 && iy < 0.0 {
        (iy / x).atan() - TRUNCATED_PI
    } else if x > 0.0 {
        (iy / x).atan()
    } else if iy < 0.0 {
        -TRUNCATED_PI / 2.0
    } else if iy > 0.0 {
        TRUNCATED_PI / 2.0
    } else {
        0.0
    }
}

/// Angle of a complex number in radians (Pascal `cang`).
pub fn cang(a: Complex64) -> f64 {
    pascal_atan2(a.re, a.im)
}

/// Angle of a complex number in degrees (Pascal `cdang`).
pub fn cdang(a: Complex64) -> f64 {
    pascal_atan2(a.re, a.im) * TRUNCATED_RAD_TO_DEG
}

/// Complex to polar, angle in radians (Pascal `ctopolar`).
pub fn c_to_polar(a: Complex64) -> Polar {
    Polar {
        mag: a.norm(),
        ang: cang(a),
    }
}

/// Complex to polar, angle in degrees (Pascal `ctopolardeg`).
pub fn c_to_polar_deg(a: Complex64) -> Polar {
    Polar {
        mag: a.norm(),
        ang: cdang(a),
    }
}

/// Polar (radians) to complex (Pascal `ptocomplex`).
pub fn p_to_complex(a: Polar) -> Complex64 {
    Complex64::new(a.mag * a.ang.cos(), a.mag * a.ang.sin())
}

/// Magnitude and angle in radians to complex (Pascal `pclx`).
pub fn pclx(mag: f64, ang: f64) -> Complex64 {
    Complex64::new(mag * ang.cos(), mag * ang.sin())
}

/// Magnitude and angle in degrees to complex (Pascal `pdegtocomplex`);
/// the degree→radian conversion divides by the truncated constant.
pub fn pdeg_to_complex(mag: f64, angle_deg: f64) -> Complex64 {
    let ang = angle_deg / TRUNCATED_RAD_TO_DEG;
    Complex64::new(mag * ang.cos(), mag * ang.sin())
}

/// Pascal `RotatePhasorDeg(Phasor, h, AngleDeg)`: rotate by `h · AngleDeg`
/// degrees (used by the harmonic injection paths).
pub fn rotate_phasor_deg(phasor: Complex64, h: f64, angle_deg: f64) -> Complex64 {
    phasor * pdeg_to_complex(1.0, h * angle_deg)
}

/// Pascal `RotatePhasorRad(Phasor, h, AngleRad)`: rotate by `h · AngleRad`
/// radians.
pub fn rotate_phasor_rad(phasor: Complex64, h: f64, angle_rad: f64) -> Complex64 {
    phasor * pclx(1.0, h * angle_rad)
}
