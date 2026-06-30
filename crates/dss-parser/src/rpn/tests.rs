use super::*;
use std::f64::consts::E;

const EPSILON: f64 = 1e-10;

#[test]
fn test_new_and_getters() {
    let calc = RPNCalculator::new();
    assert_eq!(calc.get_x(), 0.0);
    assert_eq!(calc.get_y(), 0.0);
    assert_eq!(calc.get_z(), 0.0);
}

#[test]
fn test_setters() {
    let mut calc = RPNCalculator::new();

    // set_x (should perform roll_up)
    calc.set_x(1.0);
    assert_eq!(calc.get_x(), 1.0);
    assert_eq!(calc.get_y(), 0.0);

    calc.set_x(2.0);
    assert_eq!(calc.get_x(), 2.0);
    assert_eq!(calc.get_y(), 1.0);

    // set_y and set_z (direct set)
    calc.set_y(10.0);
    calc.set_z(20.0);
    assert_eq!(calc.get_x(), 2.0);
    assert_eq!(calc.get_y(), 10.0);
    assert_eq!(calc.get_z(), 20.0);
}

#[test]
fn test_arithmetic_operations() {
    let mut calc = RPNCalculator::new();

    // 5 + 3 = 8
    calc.set_x(5.0);
    calc.set_x(3.0);
    calc.add();
    assert_eq!(calc.get_x(), 8.0);

    // 10 - 4 = 6
    calc.set_x(10.0);
    calc.set_x(4.0);
    calc.subtract();
    assert_eq!(calc.get_x(), 6.0);

    // 7 * 8 = 56
    calc.set_x(7.0);
    calc.set_x(8.0);
    calc.multiply();
    assert_eq!(calc.get_x(), 56.0);

    // 20 / 4 = 5
    calc.set_x(20.0);
    calc.set_x(4.0);
    calc.divide();
    assert_eq!(calc.get_x(), 5.0);
}

#[test]
fn test_stack_operations() {
    let mut calc = RPNCalculator::new();

    // Fill the stack
    calc.set_x(1.0);
    calc.set_x(2.0);
    calc.set_x(3.0);

    assert_eq!(calc.get_x(), 3.0);
    assert_eq!(calc.get_y(), 2.0);
    assert_eq!(calc.get_z(), 1.0);

    calc.roll_down();
    assert_eq!(calc.get_x(), 2.0);
    assert_eq!(calc.get_y(), 1.0);
    assert_eq!(calc.get_z(), 0.0);

    calc.roll_up();
    assert_eq!(calc.get_x(), 2.0);
    assert_eq!(calc.get_y(), 2.0);
    assert_eq!(calc.get_z(), 1.0);
}

#[test]
fn test_swap_xy() {
    let mut calc = RPNCalculator::new();

    calc.set_x(5.0);
    calc.set_x(10.0);
    assert_eq!(calc.get_x(), 10.0);
    assert_eq!(calc.get_y(), 5.0);

    calc.swap_xy();
    assert_eq!(calc.get_x(), 5.0);
    assert_eq!(calc.get_y(), 10.0);
}

#[test]
fn test_basic_math_functions() {
    let mut calc = RPNCalculator::new();

    // sqrt(25) = 5
    calc.set_x(25.0);
    calc.sqrt();
    assert_eq!(calc.get_x(), 5.0);

    // square(6) = 36
    calc.set_x(6.0);
    calc.square();
    assert_eq!(calc.get_x(), 36.0);

    // inv(0.5) = 2
    calc.set_x(0.5);
    calc.inv();
    assert_eq!(calc.get_x(), 2.0);

    // inv(4) = 0.25
    calc.set_x(4.0);
    calc.inv();
    assert_eq!(calc.get_x(), 0.25);
}

#[test]
fn test_power_operations() {
    let mut calc = RPNCalculator::new();

    // 2^3 = 8
    calc.set_x(2.0);
    calc.set_x(3.0);
    calc.y_to_the_x_power();
    assert_eq!(calc.get_x(), 8.0);

    // 3^4 = 81
    calc.set_x(3.0);
    calc.set_x(4.0);
    calc.y_to_the_x_power();
    assert_eq!(calc.get_x(), 81.0);

    // 10^2 = 100
    calc.set_x(10.0);
    calc.set_x(2.0);
    calc.y_to_the_x_power();
    assert_eq!(calc.get_x(), 100.0);
}

#[test]
fn test_trigonometric_functions() {
    let mut calc = RPNCalculator::new();

    // sin(0 deg) = 0
    calc.set_x(0.0);
    calc.sin_deg();
    assert!(calc.get_x().abs() < EPSILON);

    // sin(30 deg) = 0.5
    calc.set_x(30.0);
    calc.sin_deg();
    assert!((calc.get_x() - 0.5).abs() < EPSILON);

    // sin(90 deg) = 1
    calc.set_x(90.0);
    calc.sin_deg();
    assert!((calc.get_x() - 1.0).abs() < EPSILON);

    // cos(0 deg) = 1
    calc.set_x(0.0);
    calc.cos_deg();
    assert!((calc.get_x() - 1.0).abs() < EPSILON);

    // cos(60 deg) = 0.5
    calc.set_x(60.0);
    calc.cos_deg();
    assert!((calc.get_x() - 0.5).abs() < EPSILON);

    // cos(90 deg) = 0
    calc.set_x(90.0);
    calc.cos_deg();
    assert!(calc.get_x().abs() < EPSILON);

    // tan(0 deg) = 0
    calc.set_x(0.0);
    calc.tan_deg();
    assert!(calc.get_x().abs() < EPSILON);

    // tan(45 deg) = 1
    calc.set_x(45.0);
    calc.tan_deg();
    assert!((calc.get_x() - 1.0).abs() < EPSILON);
}

#[test]
fn test_inverse_trigonometric_functions() {
    let mut calc = RPNCalculator::new();

    // asin(0) = 0 deg
    calc.set_x(0.0);
    calc.asin_deg();
    assert!(calc.get_x().abs() < EPSILON);

    // asin(0.5) = 30 deg
    calc.set_x(0.5);
    calc.asin_deg();
    assert!((calc.get_x() - 30.0).abs() < EPSILON);

    // asin(1) = 90 deg
    calc.set_x(1.0);
    calc.asin_deg();
    assert!((calc.get_x() - 90.0).abs() < EPSILON);

    // acos(1) = 0 deg
    calc.set_x(1.0);
    calc.acos_deg();
    assert!(calc.get_x().abs() < EPSILON);

    // acos(0.5) = 60 deg
    calc.set_x(0.5);
    calc.acos_deg();
    assert!((calc.get_x() - 60.0).abs() < EPSILON);

    // acos(0) = 90 deg
    calc.set_x(0.0);
    calc.acos_deg();
    assert!((calc.get_x() - 90.0).abs() < EPSILON);

    // atan(0) = 0 deg
    calc.set_x(0.0);
    calc.atan_deg();
    assert!(calc.get_x().abs() < EPSILON);

    // atan(1) = 45 deg
    calc.set_x(1.0);
    calc.atan_deg();
    assert!((calc.get_x() - 45.0).abs() < EPSILON);
}

#[test]
fn test_atan2_deg() {
    let mut calc = RPNCalculator::new();

    // atan2(1, 1) = 45 deg
    calc.set_x(1.0);
    calc.set_y(1.0);
    calc.atan2_deg();
    assert!((calc.get_x() - 45.0).abs() < EPSILON);

    // atan2(1, 0) = 90 deg
    calc.set_x(0.0);
    calc.set_y(1.0);
    calc.atan2_deg();
    assert!((calc.get_x() - 90.0).abs() < EPSILON);

    // atan2(0, 1) = 0 deg
    calc.set_x(1.0);
    calc.set_y(0.0);
    calc.atan2_deg();
    assert!(calc.get_x().abs() < EPSILON);
}

#[test]
fn test_logarithmic_functions() {
    let mut calc = RPNCalculator::new();

    // ln(e) = 1
    calc.set_x(E);
    calc.nat_log();
    assert!((calc.get_x() - 1.0).abs() < EPSILON);

    // ln(1) = 0
    calc.set_x(1.0);
    calc.nat_log();
    assert!(calc.get_x().abs() < EPSILON);

    // log10(10) = 1
    calc.set_x(10.0);
    calc.ten_log();
    assert!((calc.get_x() - 1.0).abs() < EPSILON);

    // log10(100) = 2
    calc.set_x(100.0);
    calc.ten_log();
    assert!((calc.get_x() - 2.0).abs() < EPSILON);

    // log10(1) = 0
    calc.set_x(1.0);
    calc.ten_log();
    assert!(calc.get_x().abs() < EPSILON);
}

#[test]
fn test_exponential_functions() {
    let mut calc = RPNCalculator::new();

    // e^0 = 1
    calc.set_x(0.0);
    calc.etothex();
    assert!((calc.get_x() - 1.0).abs() < EPSILON);

    // e^1 = e
    calc.set_x(1.0);
    calc.etothex();
    assert!((calc.get_x() - E).abs() < EPSILON);

    // e^2 == 7.38905609893065
    calc.set_x(2.0);
    calc.etothex();
    assert!((calc.get_x() - 7.38905609893065).abs() < EPSILON);
}

#[test]
fn test_enter_pi() {
    let mut calc = RPNCalculator::new();

    // simple input of π
    calc.enter_pi();
    assert!((calc.get_x() - PI).abs() < EPSILON);

    // input of π with the stack
    calc.set_x(42.0);
    calc.enter_pi();
    assert!((calc.get_x() - PI).abs() < EPSILON);
    assert_eq!(calc.get_y(), 42.0);

    // multiple inputs of π
    calc.enter_pi();
    assert!((calc.get_x() - PI).abs() < EPSILON);
    assert!((calc.get_y() - PI).abs() < EPSILON);
    assert_eq!(calc.get_z(), 42.0);
}

#[test]
fn test_complex_calculations() {
    let mut calc = RPNCalculator::new();

    // (5 + 3) * 2 = 16
    calc.set_x(5.0);
    calc.set_x(3.0);
    calc.add(); // 8
    calc.set_x(2.0);
    calc.multiply();
    assert_eq!(calc.get_x(), 16.0);

    let mut calc = RPNCalculator::new();
    // sqrt((3^2) + (4^2)) = 5
    calc.set_x(3.0);
    calc.square(); // x = 9
    calc.set_x(4.0);
    calc.square(); // x = 16, y = 9, z = 0
    assert_eq!((calc.get_x(), calc.get_y(), calc.get_z()), (16.0, 9.0, 0.0));
    calc.add(); // x = 25, y = 9, z = 0
    assert_eq!((calc.get_x(), calc.get_y(), calc.get_z()), (25.0, 0.0, 0.0));
    calc.sqrt(); // x = 5,  y = 9, z = 0
    assert_eq!((calc.get_x(), calc.get_y(), calc.get_z()), (5.0, 0.0, 0.0));
}

#[test]
fn test_trig_identity_sin2_plus_cos2() {
    let mut calc = RPNCalculator::new();

    // sin^2(30 deg) + cos^2(30 deg) = 1
    calc.set_x(30.0);
    calc.sin_deg();
    calc.square(); // sin^(30 deg)

    calc.set_x(30.0);
    calc.cos_deg();
    calc.square(); // cos^2(30 deg)

    calc.add(); // sin^2(30 deg) + cos^2(30 deg)
    assert!((calc.get_x() - 1.0).abs() < EPSILON);
}

#[test]
fn test_logarithmic_properties() {
    let mut calc = RPNCalculator::new();

    // ln(e^x) = x for x = 2.5
    let test_value = 2.5;
    calc.set_x(test_value);
    calc.etothex(); // e^2.5
    calc.nat_log(); // ln(e^2.5) = 2.5
    assert!((calc.get_x() - test_value).abs() < EPSILON);

    // 10^(log10(x)) = x for x = 123.456
    let test_value = 123.456;
    calc.set_x(test_value);
    calc.ten_log(); // log10(123.456)
    calc.set_x(10.0);
    calc.swap_xy();
    calc.y_to_the_x_power(); // 10^(log10(123.456)) = 123.456
    assert!((calc.get_x() - test_value).abs() < 1e-10);
}

#[test]
fn test_edge_cases() {
    let mut calc = RPNCalculator::new();

    // sqrt(0) = 0
    calc.set_x(0.0);
    calc.sqrt();
    assert_eq!(calc.get_x(), 0.0);

    // 0^0
    calc.set_x(0.0);
    calc.set_x(0.0);
    calc.y_to_the_x_power();
    assert_eq!(calc.get_x(), 1.0);

    // ln(1) = 0
    calc.set_x(1.0);
    calc.nat_log();
    assert_eq!(calc.get_x(), 0.0);
}

#[test]
fn test_stack_depth() {
    let mut calc = RPNCalculator::new();

    // Fill the entire stack
    for i in 1..=10 {
        calc.set_x(i as f64);
    }

    // Check that the last values are in place
    assert_eq!(calc.get_x(), 10.0);
    assert_eq!(calc.get_y(), 9.0);
    assert_eq!(calc.get_z(), 8.0);

    // Check that values are not lost when overflowing
    for _ in 0..7 {
        calc.roll_down();
    }
    assert_eq!(calc.get_x(), 3.0);
    assert_eq!(calc.get_y(), 2.0);
    assert_eq!(calc.get_z(), 1.0);
}
