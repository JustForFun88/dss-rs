//! Unit tests for the voltage-scaled power-comparison floor
//! (`harness::assert_power_close`). The live/golden gates exercise it end-to-end,
//! but its contract — the power abs floor is the *image of the current floor
//! through the terminal voltage*, `i_abs · max(1, |V_kv|)` with `|V_kv|=|P|/|I|` —
//! is otherwise pinned only by documentation. These cases lock the scaling rule
//! directly (no oracle), in both directions: it loosens **with** voltage, never
//! unconditionally. See tests/TOLERANCE_NOTES.md.

mod harness;

use harness::{ElementCap, assert_power_close};
use num_complex::Complex64;

/// One single-conductor terminal: real current `i_re` A, real power `p_kw` kW, so
/// the recovered terminal voltage is `|V_kv| = p_kw / i_re`.
fn cap(i_re: f64, p_kw: f64) -> ElementCap {
    ElementCap {
        name: "line.test".to_string(),
        i_re: vec![i_re],
        i_im: vec![0.0],
        p_kw: vec![p_kw],
        p_kvar: vec![0.0],
        loss_w: vec![],
    }
}

const REL: f64 = 1e-6;
const ABS: f64 = 1e-4;

// 10 A, 100 kW -> |V_kv| = 10 kV terminal. abs floor = 1e-4·10 = 1e-3 kW, so a
// 1.0e-3 kW power error (10× the flat 1e-4 floor) is the image of a sub-1e-4-A
// current error and PASSES — the floor scales up with terminal voltage.
#[test]
fn power_floor_scales_up_at_high_voltage() {
    let exp = cap(10.0, 100.0);
    let actual = vec![Complex64::new(100.0 + 1.0e-3, 0.0)];
    assert_power_close(&actual, &exp, REL, ABS, "hv");
}

// Same high-V terminal, but an error *above* the scaled floor (1.3e-3 > 1.1e-3 =
// 1e-4·10 + 1e-6·100) must still FAIL — the scaling is bounded, not a blank check.
#[test]
#[should_panic(expected = "differs")]
fn power_floor_rejects_error_above_scaled_floor() {
    let exp = cap(10.0, 100.0);
    let actual = vec![Complex64::new(100.0 + 1.3e-3, 0.0)];
    assert_power_close(&actual, &exp, REL, ABS, "hv-over");
}

// 10 A, 1 kW -> |V_kv| = 0.1 kV terminal; `max(1, 0.1)` keeps the floor at the
// established 1e-4 kW. The SAME 1.0e-3 kW error that passed at 10 kV now FAILS
// here — proving the floor is not unconditionally permissive (never tightens
// below 1e-4, never loosens below |V_kv| = 1).
#[test]
#[should_panic(expected = "differs")]
fn power_floor_unscaled_below_unity_voltage_rejects_same_error() {
    let exp = cap(10.0, 1.0);
    let actual = vec![Complex64::new(1.0 + 1.0e-3, 0.0)];
    assert_power_close(&actual, &exp, REL, ABS, "lv");
}
