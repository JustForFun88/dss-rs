//! Polar/angle helpers, port of `Shared/DSSUcomplex.pas`.
//!
//! Most of that unit maps directly onto `num_complex` (`cmplx` → `Complex64::new`,
//! `cabs` → `norm`, `cabs2` → `norm_sqr`, `cong` → `conj`, `cZERO`/`cONE` →
//! `Complex64::ZERO`/`ONE`) and is not re-wrapped. What lives here are the
//! polar-form conversions, which deliberately reproduce the original's
//! truncated constants: angles in degrees are scaled by `57.29577951`, and
//! the quadrant correction in `cang` adds the truncated `3.14159265359` —
//! both visible in engine report output.

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

// The truncated constants from DSSUcomplex.pas, preserved on purpose.
#[allow(clippy::approx_constant)]
const TRUNCATED_PI: f64 = 3.14159265359;
const TRUNCATED_RAD_TO_DEG: f64 = 57.29577951;

/// The private `ATAN2(x, iy)` from DSSUcomplex.pas: a hand-rolled atan2 whose
/// quadrant correction uses the truncated pi. Argument order is (re, im).
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

#[cfg(test)]
mod tests {
    use super::*;

    fn c(re: f64, im: f64) -> Complex64 {
        Complex64::new(re, im)
    }

    #[test]
    fn cang_quadrants_match_pascal_branches() {
        // first quadrant: plain arctan
        assert_eq!(cang(c(1.0, 1.0)), (1.0f64).atan());
        // second quadrant: + truncated pi
        assert_eq!(cang(c(-1.0, 1.0)), (-1.0f64).atan() + TRUNCATED_PI);
        // third quadrant: - truncated pi
        assert_eq!(cang(c(-1.0, -1.0)), (1.0f64).atan() - TRUNCATED_PI);
        // pure imaginary
        assert_eq!(cang(c(0.0, 2.0)), TRUNCATED_PI / 2.0);
        assert_eq!(cang(c(0.0, -2.0)), -TRUNCATED_PI / 2.0);
        // zero
        assert_eq!(cang(c(0.0, 0.0)), 0.0);
    }

    #[test]
    fn cang_differs_from_std_atan2_by_truncation_only() {
        let v = c(-3.0, 4.0);
        let diff = (cang(v) - v.im.atan2(v.re)).abs();
        assert!(diff > 0.0, "the truncated pi must be visible");
        assert!(diff < 1e-11, "but only at the truncation scale: {diff}");
    }

    #[test]
    fn cdang_uses_truncated_scale() {
        // 45 degrees: atan(1) * 57.29577951
        let got = cdang(c(1.0, 1.0));
        assert_eq!(got, (1.0f64).atan() * 57.29577951);
        assert!((got - 45.0).abs() < 1e-7);
    }

    #[test]
    fn polar_round_trip() {
        let v = c(3.0, -4.0);
        let p = c_to_polar(v);
        assert_eq!(p.mag, 5.0);
        let back = p_to_complex(p);
        assert!((back - v).norm() < 1e-11); // truncated pi limits the precision
    }

    #[test]
    fn pdeg_round_trip_with_truncated_conversion() {
        let v = pdeg_to_complex(10.0, 30.0);
        // forward conversion uses deg / 57.29577951
        let ang: f64 = 30.0 / 57.29577951;
        assert_eq!(v, c(10.0 * ang.cos(), 10.0 * ang.sin()));
        let p = c_to_polar_deg(v);
        assert!((p.mag - 10.0).abs() < 1e-12);
        assert!((p.ang - 30.0).abs() < 1e-9);
    }

    #[test]
    fn pclx_radians() {
        let v = pclx(2.0, std::f64::consts::FRAC_PI_2);
        assert!((v - c(0.0, 2.0)).norm() < 1e-15);
        assert_eq!(to_polar(2.0, 0.5), Polar { mag: 2.0, ang: 0.5 });
    }
}
