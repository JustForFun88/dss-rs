//! Pascal number formatters shared across the Phase-8 reports — the
//! `Format('%…g', …)` / `Format('%…f', …)` call sites in `ExportResults.pas`
//! and `ShowResults.pas` (Pascal `Common/Utilities.pas` number helpers).
//!
//! The text/CSV golden gate parses numbers out (`harness::compare_export`,
//! PHASE8_PLAN §2.3), so these only need to be *faithful* (notation + digit
//! count), not bit-exact to FPC's formatter — the value comparison is by
//! tolerance. `%g` delegates to the existing C-`printf`-style
//! [`crate::util::fmt_g`].

use crate::util::fmt_g;

/// Pascal `Format('%.*g', [sig, v])`: `sig` significant digits, the `%g` notation
/// rules (see [`fmt_g`]). The field-width / justification prefixes in the Pascal
/// specs (`%10.6g`, `%-13.11g`, …) only space-pad, which the comparator trims, so
/// they are not reproduced here.
///
/// Note the one byte-level difference from C `printf`: FPC `Format`'s `%g`/`%e`
/// conversions emit an **uppercase** `E` exponent (`1.19304E-6`), where C's `%g`
/// emits lowercase `e`. [`fmt_g`] follows C, so we uppercase the exponent to match
/// the oracle (only visible in a byte-exact report — the value-parsing gate is
/// case-blind; surfaced by `Show LineConstants`' scientific susceptance cells).
pub fn g(v: f64, sig: usize) -> String {
    // `fmt_g` output is a pure number token (digits, `.`, `+`/`-`, `e`), so the
    // only `e` is the exponent marker — the uppercase swap is unambiguous.
    fmt_g(v, sig).replace('e', "E")
}

/// Pascal `Format('%.*f', [decimals, v])`: fixed-point with `decimals`
/// fractional digits (`%6.1f`, `%8.2f`, …; the width pad is dropped).
pub fn fixed(v: f64, decimals: usize) -> String {
    format!("{v:.decimals$}")
}

/// Pascal `Format('%W.Df', [v])`: fixed-point, `decimals` fractional digits,
/// right-justified (space-padded) in field width `width`. The `Show` reports are
/// fixed-width tables, so — unlike the CSV exports — the width matters: it keeps a
/// numeric field from gluing to a neighbour when a value doesn't fill it (the
/// text/CSV comparator collapses the padding, but the separation must exist).
///
/// This uses Rust's native `{:.N}` (round-half-to-even on the true `f64`), which
/// is *faithful* for the value-parsed Show tables — and, since F.4, it is also
/// the **default kernel** the whole-circuit JSON PostCommands render through
/// ([`crate::compat::fixed_w_script`]); the parity lane keeps the byte-exact
/// [`fixed_w_fpc_impl`] there.
pub fn fixed_w(v: f64, width: usize, decimals: usize) -> String {
    format!("{v:>width$.decimals$}")
}

/// FPC `Format('%W.Df', [v])` — **byte-exact** to the FPC RTL `FloatToStrF(ffFixed)`
/// path used by the AltDSS whole-circuit PostCommands (`Set ueweight=%8.2f`,
/// `Set lossweight=%8.2f`, `CAPI_Obj.pas:2593-2594`). Distinct from [`fixed_w`]:
/// FPC first renders the value at **15 significant digits** (its `FloatToDecimal`
/// precision for a `Double`), then rounds that decimal to `decimals` fractional
/// digits with **ties-away-from-zero**. Rust's native `{:.N}` differs on both
/// counts, so at a rounding boundary they diverge — e.g. `0.125 -> 0.13` (not the
/// ties-to-even `0.12`), `2.675 -> 2.68` (the 15-sig intermediate is
/// `2.67500000000000`, not the true `2.6749999…` that rounds to `2.67`),
/// `99999.995 -> 100000.00`. Empirically pinned against the pinned oracle
/// (`tools/golden/gen_json.py` `circuit_positive_seq`).
///
/// **The parity kernel of the F-FMT seam's fixed renderer**
/// ([`crate::compat::fixed_w_script`]). The two-stage decimal rounding it
/// reproduces is the row's deliberate inexactness; the default lane selects
/// [`fixed_w`], which is the single correctly-rounded fixed format that
/// inexactness stands in for. Both stay compiled and are asserted against each
/// other by this module's tests.
pub fn fixed_w_fpc_impl(v: f64, width: usize, decimals: usize) -> String {
    let s = fpc_fixed(v, decimals);
    format!("{s:>width$}")
}

/// Core of [`fixed_w_fpc_impl`] without the width padding: the FPC `ffFixed` string.
fn fpc_fixed(v: f64, decimals: usize) -> String {
    if !v.is_finite() {
        // Inf/NaN — unreachable for the circuit weights; defer to Rust.
        return format!("{v:.decimals$}");
    }
    let negative = v.is_sign_negative();
    // FPC's `FloatToDecimal` renders a `Double` at 15 significant digits; obtain
    // that intermediate via Rust's correctly-rounded scientific format
    // (1 leading + 14 fraction digits = 15 significant).
    let sci = format!("{:.14e}", v.abs()); // "d.ddddddddddddddeE"
    let (mant, exp_s) = sci.split_once('e').expect("scientific has exponent");
    let exp: i32 = exp_s.parse().expect("valid exponent");
    let digits: Vec<u8> = mant
        .bytes()
        .filter(u8::is_ascii_digit)
        .map(|b| b - b'0')
        .collect(); // 15 digits, MSB first; digits[0] sits at place 10^exp.

    // Split the 15-digit string at the decimal point (`point` integer digits).
    let point = exp + 1;
    let mut int_digits: Vec<u8> = Vec::new();
    let mut frac_digits: Vec<u8> = Vec::new();
    if point <= 0 {
        // 0.00…d1d2…: |point| leading fractional zeros, then all 15 digits.
        frac_digits.extend(std::iter::repeat_n(0u8, (-point) as usize));
        frac_digits.extend_from_slice(&digits);
    } else if point as usize >= digits.len() {
        int_digits.extend_from_slice(&digits);
        int_digits.extend(std::iter::repeat_n(0u8, point as usize - digits.len()));
    } else {
        int_digits.extend_from_slice(&digits[..point as usize]);
        frac_digits.extend_from_slice(&digits[point as usize..]);
    }

    // Round the fractional part to `decimals` places, ties-away-from-zero: the
    // magnitude is non-negative here, so "away from zero" == round the first
    // dropped digit `>= 5` up.
    let round_up = frac_digits.len() > decimals && frac_digits[decimals] >= 5;
    frac_digits.truncate(decimals);
    while frac_digits.len() < decimals {
        frac_digits.push(0);
    }
    if round_up {
        let mut carry = 1u8;
        for d in frac_digits.iter_mut().rev() {
            let x = *d + carry;
            *d = x % 10;
            carry = x / 10;
            if carry == 0 {
                break;
            }
        }
        if carry != 0 {
            for d in int_digits.iter_mut().rev() {
                let x = *d + carry;
                *d = x % 10;
                carry = x / 10;
                if carry == 0 {
                    break;
                }
            }
            if carry != 0 {
                int_digits.insert(0, carry);
            }
        }
    }
    if int_digits.is_empty() {
        int_digits.push(0);
    }

    let all_zero = int_digits.iter().all(|&d| d == 0) && frac_digits.iter().all(|&d| d == 0);
    let mut out = String::new();
    if negative && !all_zero {
        out.push('-');
    }
    for &d in &int_digits {
        out.push((b'0' + d) as char);
    }
    if decimals > 0 {
        out.push('.');
        for &d in &frac_digits {
            out.push((b'0' + d) as char);
        }
    }
    out
}

/// Pascal `Format('%Wd', [v])`: integer, right-justified (space-padded) in field
/// width `width`.
pub fn fixed_w_int(v: i64, width: usize) -> String {
    format!("{v:>width$}")
}

/// Pascal `Format('%W.Pg', [v])`: `%g` with `sig` significant digits, right-
/// justified in field width `width`. Uppercase-`E` exponent (see [`g`]).
pub fn g_w(v: f64, width: usize, sig: usize) -> String {
    let s = g(v, sig);
    format!("{s:>width$}")
}

/// Pascal `Format('%-W.Pg', [v])`: `%g` with `sig` significant digits,
/// **left**-justified in field width `width` (the `ShowBuses` coordinate columns).
/// Uppercase-`E` exponent (see [`g`]).
pub fn g_left_w(v: f64, width: usize, sig: usize) -> String {
    let s = g(v, sig);
    format!("{s:<width$}")
}

/// Pascal FPC `Str(v: width)` (the default real-to-string with *only* a field
/// width): scientific notation with `width - 8` fraction digits (floored at 1)
/// and a signed, ≥3-digit zero-padded exponent, an explicit sign slot (`' '`
/// for non-negative, `'-'` for negative — part of the *minimum* representation,
/// not incidental padding), the whole right-justified in `width` (`6.639353E
/// +004` → ` 6.639353E+004` at width 14; probe-confirmed the sign slot survives
/// even at width 0 — Monitor's `// Sec=` `WriteStr(sout, '// Sec=', Sec: 0)`
/// renders `0.0` as `// Sec= 0.0E+000`, the 9-char floor `frac=1` + sign). The
/// convergence report's `|V|`/`Vbase` columns write `VmagSaved: 14` /
/// `NodeVbase: 14` (`Solution.pas` `WriteConvergenceReport`). Probe-matched to
/// the pinned oracle's bytes; the value comparison in the golden is by parsed
/// value, so any last-digit rounding difference (FPC-vs-Rust `{:E}`) is
/// absorbed by the column's printing-floor tol.
pub fn fpc_sci_w(v: f64, width: usize) -> String {
    let body = fpc_sci_body(v, width);
    format!("{body:>width$}")
}

/// [`fpc_sci_w`] without the right-justification — the *minimum* FPC `Str(v: w)`
/// representation (sign slot, mantissa, `E±ddd`), whose fraction-digit count is
/// still set by `width`. Split out for the `Show` table model, where the field
/// width belongs to the [`crate::report::table::Cell`] and the padding is the
/// lane's ([`crate::compat::render_rows`]); `fpc_sci_w` = this body in a
/// `width`-wide right-justified field, byte for byte.
pub fn fpc_sci_body(v: f64, width: usize) -> String {
    let frac = ((width as i64) - 8).max(1) as usize;
    let sign = if v.is_sign_negative() { '-' } else { ' ' };
    // Rust `{:.*E}` → `6.639353E4`; reformat the exponent to a sign plus an
    // at-least-3-digit zero-padded magnitude (sign handled explicitly above).
    let s = format!("{:.frac$E}", v.abs());
    let (mant, exp) = s.split_once('E').unwrap_or((s.as_str(), "0"));
    let (esign, edig) = match exp.strip_prefix('-') {
        Some(r) => ('-', r),
        None => ('+', exp.trim_start_matches('+')),
    };
    format!("{sign}{mant}E{esign}{edig:0>3}")
}

/// Pascal `Pad(S, Width)` (`Common/Utilities.pas`): `S` right-padded with **spaces**
/// to `Width` chars; a string already `>= Width` is returned unchanged. Uses byte
/// length (`str::len`), matching Pascal's `Length(S)` (byte-1:1).
pub fn pad(s: &str, width: usize) -> String {
    if s.len() >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - s.len()))
    }
}

/// Pascal `PadDots(S, Width)` (`Common/ShowResults.pas`): `S` right-padded with
/// **dots** to `Width` chars (the leading-dot padding string starts with a space,
/// `' ....'`, so the first pad char is a space then dots — reproduced verbatim).
pub fn pad_dots(s: &str, width: usize) -> String {
    // Pascal `paddotsString = ' .................................................'`
    // (a leading space then 49 dots); `Copy(paddotsString, 1, Width-Len(S))`.
    const PAD_DOTS: &str = " .................................................";
    let len = s.len(); // byte length, Pascal `Length(S)`
    if len >= width {
        s.to_string()
    } else {
        let n = width - len;
        format!("{s}{}", &PAD_DOTS[..n.min(PAD_DOTS.len())])
    }
}

/// Pascal `EncloseQuotes(s)` (`Common/Utilities.pas`): `"` + `s` + `"`.
pub fn enclose_quotes(s: &str) -> String {
    format!("\"{s}\"")
}

/// Pascal `StripExtension(S)` (`Common/Utilities.pas`): everything up to the first
/// `.` (a bus name with its `.node.node…` suffix removed); no `.` → `S` unchanged.
pub fn strip_extension(s: &str) -> String {
    match s.find('.') {
        Some(p) => s[..p].to_string(),
        None => s.to_string(),
    }
}

/// Pascal `DSSClassName + '.' + AnsiUpperCase(Name)` (`ExportResults.pas`): the
/// element full name with **only the element-name part uppercased**, the class
/// name left in its native case (e.g. `Transformer.SUB`, `Line.650632`). The
/// element name is a single token, so split on the first `.`.
pub fn upper_elem_name(full_name: &str) -> String {
    match full_name.split_once('.') {
        Some((cls, name)) => format!("{cls}.{}", name.to_uppercase()),
        None => full_name.to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// [`fixed_w_fpc_impl`] must reproduce FPC `Format('%8.2f')` byte-for-byte.
    /// Every pair below is a value fed to the pinned oracle's circuit `ueweight`
    /// PostCommand and the exact width-8 string it emitted — the divergence from
    /// Rust's native `{:>8.2}` (ties-to-even on the true f64) is real and reachable
    /// (a fractional weight at a rounding boundary).
    ///
    /// Asserted against the **impl**, not the seam, so it pins the parity kernel
    /// in *both* lanes (the F-FMT mechanism: both kernels always compiled).
    #[test]
    fn fixed_w_fpc_matches_oracle_percent_8_2f() {
        // (input weight, oracle `Set ueweight=` value with its width-8 padding)
        let cases: &[(f64, &str)] = &[
            (1.0, "    1.00"),
            (0.125, "    0.13"),
            (0.135, "    0.14"),
            (0.145, "    0.15"),
            (0.155, "    0.16"),
            (2.675, "    2.68"),
            (2.665, "    2.67"),
            (0.005, "    0.01"),
            (0.015, "    0.02"),
            (0.025, "    0.03"),
            (0.045, "    0.05"),
            (0.055, "    0.06"),
            (1.005, "    1.01"),
            (1.015, "    1.02"),
            (123.455, "  123.46"),
            (123.465, "  123.47"),
            (0.001, "    0.00"),
            (0.994999, "    0.99"),
            (0.995, "    1.00"),
            (0.9999, "    1.00"),
            (10.005, "   10.01"),
            (0.375, "    0.38"),
            (0.625, "    0.63"),
            (0.875, "    0.88"),
            (1234.565, " 1234.57"),
            (0.0049999, "    0.00"),
            (99999.995, "100000.00"),
            (0.105, "    0.11"),
            (0.115, "    0.12"),
        ];
        for &(v, want) in cases {
            assert_eq!(fixed_w_fpc_impl(v, 8, 2), want, "fixed_w_fpc_impl({v})");
        }
    }

    #[test]
    fn fixed_w_fpc_zero_and_sign() {
        assert_eq!(fixed_w_fpc_impl(0.0, 8, 2), "    0.00");
        assert_eq!(fixed_w_fpc_impl(-0.0, 8, 2), "    0.00"); // no `-0.00`
        assert_eq!(fixed_w_fpc_impl(-0.005, 8, 2), "   -0.01"); // ties-away, sign kept
    }

    /// The F-FMT fixed row, at its observable: the seam
    /// [`crate::compat::fixed_w_script`] resolves to FPC's two-stage rounding in
    /// the parity lane and to the single correctly-rounded [`fixed_w`] in the
    /// default lane — and the two genuinely disagree, so this is a real split
    /// and not a rename.
    ///
    /// The expected values are the *documented* difference between the rules:
    /// `0.125` is an exact binary half, so ties-away gives `0.13` and Rust's
    /// ties-to-even gives `0.12`; `2.675` is really `2.67499999999999982…`, which
    /// FPC's 15-significant intermediate (`2.67500000000000`) rounds up to `2.68`
    /// while a single correct rounding of the true value gives `2.67`.
    #[test]
    fn fixed_w_script_is_the_lane_kernel() {
        use crate::compat::{ORACLE_PARITY, fixed_w_script};
        for (v, parity, default) in [
            (0.125_f64, "    0.13", "    0.12"),
            (2.675, "    2.68", "    2.67"),
            (99999.995, "100000.00", "99999.99"),
        ] {
            assert_eq!(
                fixed_w_script(v, 8, 2),
                if ORACLE_PARITY { parity } else { default },
                "fixed_w_script({v}) in the {} lane",
                if ORACLE_PARITY { "parity" } else { "default" }
            );
            assert_ne!(parity, default, "the two kernels must actually differ");
        }
        // Away from a boundary the lanes agree — the split is exactly the
        // rounding rule, not a different renderer.
        assert_eq!(fixed_w_script(1.0, 8, 2), "    1.00");
        assert_eq!(fixed_w_script(123.4, 8, 2), "  123.40");
    }
}
