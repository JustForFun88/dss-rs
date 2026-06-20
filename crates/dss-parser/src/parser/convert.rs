//! FPC `Val`/`Round` number conversions used by the token converters, verified
//! empirically against the reference engine (`tools/golden/probe_val.py`).

/// FPC `Val` for doubles. Rust's `f64::from_str` matches it on every probed
/// case except the verbose `infinity` spelling, which is rejected here.
///
/// TODO(compat): the `infinity` rejection only mirrors FPC's narrower
/// grammar; collapse to plain `f64::from_str` once the 1:1 port is complete.
pub fn val_f64(s: &str) -> Option<f64> {
    let t = s.strip_prefix(['+', '-']).unwrap_or(s);
    if t.eq_ignore_ascii_case("infinity") {
        return None;
    }
    s.parse::<f64>().ok()
}

/// FPC `Val` for integers: optional sign, then `$`/`0x`/`0X` hex, `%` binary,
/// `&` octal, or decimal. Out-of-range values fail (range check), letting the
/// caller fall back to float conversion.
pub fn val_i32(s: &str) -> Option<i32> {
    let (neg, rest) = match s.as_bytes().first()? {
        b'+' => (false, &s[1..]),
        b'-' => (true, &s[1..]),
        _ => (false, s),
    };
    let (radix, digits) = if let Some(h) = rest.strip_prefix('$') {
        (16, h)
    } else if let Some(h) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        (16, h)
    } else if let Some(b) = rest.strip_prefix('%') {
        (2, b)
    } else if let Some(o) = rest.strip_prefix('&') {
        (8, o)
    } else {
        (10, rest)
    };
    if digits.is_empty() {
        return None;
    }
    let magnitude = i64::from_str_radix(digits, radix).ok()?;
    let value = if neg { -magnitude } else { magnitude };
    i32::try_from(value).ok()
}

/// FPC `Round`: round-to-nearest-even to Int64 (x87/SSE default mode; out of
/// range and non-finite give the "integer indefinite" `i64::MIN`), then
/// truncated to i32 like the Pascal `Integer := Round(...)` assignment.
///
/// TODO(compat): the integer-indefinite path (`inf`/`nan`/overflow → wrapped
/// `i64::MIN`, e.g. "inf" → 0) reproduces an FPC/x86 implementation artifact
/// verified via probe_val.py; make it a proper error once the 1:1 port is
/// complete.
pub(super) fn pascal_round_to_i32(x: f64) -> i32 {
    let r = x.round_ties_even();
    let wide = if r >= -(2f64.powi(63)) && r < 2f64.powi(63) {
        r as i64 // r is finite here: NaN comparisons are false
    } else {
        i64::MIN
    };
    wide as i32
}
