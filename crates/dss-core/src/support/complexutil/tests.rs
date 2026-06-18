
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
