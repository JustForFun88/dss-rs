//! Per-kernel tests of the `dss-parser` half of the Stage F lane split. Both
//! `*_impl`s are compiled in every build, so these pin the documented bounds in
//! **both** lanes.

use super::*;

/// The pi row **is** flipped (F.3d): the alias resolves to the truncated Pascal
/// literal under `oracle-parity` and to `f64::consts::PI` otherwise. Asserted
/// against both impls, which genuinely differ, so neither lane can pass
/// vacuously.
#[test]
fn pi_alias_is_the_lane_kernel() {
    assert_ne!(PI_FPC_TRUNCATED_IMPL, PI_STD_IMPL);

    let expected = if ORACLE_PARITY {
        PI_FPC_TRUNCATED_IMPL
    } else {
        PI_STD_IMPL
    };
    assert_eq!(PI, expected);
}

/// The round row **is** flipped: the alias must resolve to the parity kernel
/// under `oracle-parity` and to the saturating one otherwise — asserted on
/// `inf`, where the two impls disagree (0 vs `i32::MAX`), so neither lane can
/// pass by accident.
///
/// This doubles as the feature-propagation guard for this crate: if
/// `dss-core/oracle-parity → dss-parser/oracle-parity` ever stopped
/// propagating, [`ORACLE_PARITY`] would go false in a parity build and the
/// alias would silently become the default kernel — caught here.
#[test]
fn round_alias_is_the_lane_kernel() {
    assert_eq!(round_i32_fpc_impl(f64::INFINITY), 0);
    assert_eq!(round_i32_saturating_impl(f64::INFINITY), i32::MAX);

    let expected = if ORACLE_PARITY { 0 } else { i32::MAX };
    assert_eq!(round_i32(f64::INFINITY), expected);
}

/// The round row's **array** kernel — Pascal's `ApplyRound`, which writes
/// `Round`'s Int64 back into the Double. Pinned on `1e20`, which the pinned
/// oracle reports as `-9.22337203685478E18` (= `i64::MIN as f64`) while the
/// default lane keeps the magnitude; both impls are asserted directly, so
/// neither lane passes vacuously.
#[test]
fn round_f64_alias_is_the_lane_kernel() {
    const SENTINEL: f64 = -9_223_372_036_854_775_808.0; // i64::MIN as f64
    assert_eq!(round_f64_fpc_impl(1e20), SENTINEL);
    assert_eq!(round_f64_native_impl(1e20), 1e20);
    assert_eq!(round_f64_fpc_impl(f64::INFINITY), SENTINEL);
    assert_eq!(round_f64_native_impl(f64::INFINITY), f64::INFINITY);

    let expected = if ORACLE_PARITY { SENTINEL } else { 1e20 };
    assert_eq!(round_f64(1e20), expected);
}

/// The two FPC-lane kernels of the round row are the same `Round`, differing
/// only in the Pascal assignment target: `Integer` truncates, `Double` widens.
#[test]
fn round_f64_and_round_i32_share_the_fpc_kernel() {
    for x in [0.0, 2.5, -2.5, 3.7, 1e9, -1e9, 1e20, -1e20, f64::NAN] {
        let wide = round_f64_fpc_impl(x);
        assert_eq!(
            round_i32_fpc_impl(x),
            wide as i64 as i32,
            "the i32 kernel must be the i64 kernel truncated, at {x:e}"
        );
        // In range, both lanes agree and the value is just rounded in place.
        if x.abs() < 1e18 && x.is_finite() {
            assert_eq!(wide, round_f64_native_impl(x));
        }
    }
}

/// Re-measured 2026-08-01: the shortened literal sits **2.069e-13 above**
/// `std::f64::consts::PI` (6.59e-14 relative) — `3.14159265359` is pi rounded
/// up at the 12th significant digit, not truncated down. That direction is the
/// documented parity-vs-default bound of the pi row, and it is why `"30 sin"`
/// prints `0.5000000000000299` — *larger* than the true `0.5` — in the parity
/// lane. (The earlier record said "below"; the assertion uses `.abs()`, so
/// nothing failed, but the sign is what a reader reasons from.)
#[test]
fn pi_impls_agree_to_the_truncation_of_the_pascal_literal() {
    let abs = (PI_STD_IMPL - PI_FPC_TRUNCATED_IMPL).abs();
    assert!(
        (2.0e-13..2.2e-13).contains(&abs),
        "pi truncation gap moved: {abs:e}"
    );
    // The Pascal literal rounds pi *up*; a change that made it smaller would
    // invert the documented direction of every parity-lane trig result.
    const { assert!(PI_FPC_TRUNCATED_IMPL > PI_STD_IMPL) };
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
