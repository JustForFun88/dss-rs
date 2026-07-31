//! FPC `Val`/`Round` number conversions used by the token converters, verified
//! empirically against the reference engine (`tools/golden/probe_val.py`).

/// FPC `Val` for doubles. Rust's `f64::from_str` matches it on every probed
/// case except the verbose `infinity` spelling, which is rejected here.
///
/// The rejection stays in **both** lanes: which literals the deck language
/// accepts is command-input semantics, not a numeric kernel — the product
/// contract is that a deck parses the same everywhere, and any widening or
/// tightening of the accepted grammar belongs to the strict-parsing layer
/// (`DE_PASCALIZE_PLAN.md` IV.1b, explicitly sequenced *after* Stage F), where
/// it can be opt-in and diagnosed. Note `f64::from_str` would map `infinity`
/// to `inf`, so "collapsing" it here would silently turn a conversion error
/// into a non-finite property value.
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

/// `Round(x)` assigned to a Pascal `Integer`, through the Stage F lane seam:
/// the parity kernel reproduces FPC's integer-indefinite artifact, the default
/// kernel saturates — see [`crate::compat::round_i32`].
pub(super) fn pascal_round_to_i32(x: f64) -> i32 {
    crate::compat::round_i32(x)
}
