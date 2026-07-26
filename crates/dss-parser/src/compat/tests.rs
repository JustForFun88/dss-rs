//! Per-kernel tests of the `dss-parser` half of the Stage F lane split. Both
//! `*_impl`s are compiled in every build, so these pin the documented bounds in
//! **both** lanes.

use super::*;

/// F.1 lands the seam bit-neutral: both aliases select the parity kernel in
/// either lane. Each assertion uses a value on which the two impls genuinely
/// differ, so it cannot pass vacuously. F.3 rewrites this into per-lane
/// expectations.
#[test]
fn f1_staging_every_alias_selects_the_parity_kernel_in_both_lanes() {
    assert_eq!(PI, PI_FPC_TRUNCATED_IMPL);
    assert_ne!(PI_FPC_TRUNCATED_IMPL, PI_STD_IMPL);

    assert_eq!(round_i32(f64::INFINITY), 0);
    assert_eq!(round_i32_saturating_impl(f64::INFINITY), i32::MAX);
}

/// Measured 2026-07-26: the truncated literal sits **2.069e-13** below
/// `std::f64::consts::PI` (6.59e-14 relative) — the documented
/// parity-vs-default bound of the pi row, and the reason `"30 sin"` prints
/// `0.5000000000000299` in the parity lane.
#[test]
fn pi_impls_agree_to_the_truncation_of_the_pascal_literal() {
    let abs = (PI_STD_IMPL - PI_FPC_TRUNCATED_IMPL).abs();
    assert!(
        (2.0e-13..2.2e-13).contains(&abs),
        "pi truncation gap moved: {abs:e}"
    );
    let rel = abs / PI_STD_IMPL;
    assert!(rel < 1.0e-13, "pi relative gap: {rel:e}");

    // Its one observable consequence, the degree→radian scaling of `sin`.
    let sin30_parity = (PI_FPC_TRUNCATED_IMPL / 180.0 * 30.0).sin();
    let sin30_std = (PI_STD_IMPL / 180.0 * 30.0).sin();
    assert_eq!(sin30_parity, 0.5000000000000299);
    assert_eq!(sin30_std, 0.49999999999999994);
}

#[test]
fn round_impls_agree_on_every_in_range_finite_value() {
    for x in [
        0.0,
        -0.0,
        0.5,
        1.5,
        2.5,
        -0.5,
        -1.5,
        -2.5,
        0.49999999999999994,
        3.7,
        -3.7,
        1e9,
        -1e9,
        2147483647.0,
        -2147483648.0,
    ] {
        assert_eq!(
            round_i32_fpc_impl(x),
            round_i32_saturating_impl(x),
            "in-range value {x} must round identically"
        );
    }
}

/// The deliberate divergence of the round row, in its two classes: the FPC
/// "integer indefinite" artifact (`i64::MIN`, truncated to i32 = 0) beyond
/// Int64 range, and the plain i64→i32 truncation wrap in between — against a
/// saturating cast in both.
#[test]
fn round_impls_diverge_beyond_i32_range() {
    // (a) beyond Int64 range → FPC's integer indefinite.
    for x in [f64::INFINITY, 1e300, 2.0f64.powi(63)] {
        assert_eq!(round_i32_fpc_impl(x), 0);
        assert_eq!(round_i32_saturating_impl(x), i32::MAX);
    }
    for x in [f64::NEG_INFINITY, -1e300, -2.0f64.powi(64)] {
        assert_eq!(round_i32_fpc_impl(x), 0);
        assert_eq!(round_i32_saturating_impl(x), i32::MIN);
    }

    // (b) inside Int64 but outside i32 → the Pascal `Integer := Round(...)`
    // assignment wraps; the idiomatic cast saturates.
    assert_eq!(round_i32_fpc_impl(1e10), 1_410_065_408);
    assert_eq!(round_i32_saturating_impl(1e10), i32::MAX);
    assert_eq!(round_i32_fpc_impl(-1e10), -1_410_065_408);
    assert_eq!(round_i32_saturating_impl(-1e10), i32::MIN);

    // NaN lands on 0 either way.
    assert_eq!(round_i32_fpc_impl(f64::NAN), 0);
    assert_eq!(round_i32_saturating_impl(f64::NAN), 0);
}
