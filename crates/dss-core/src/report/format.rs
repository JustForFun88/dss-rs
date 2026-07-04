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

/// Pascal `Format('%.*g', [sig, v])`: `sig` significant digits, the C `%g`
/// notation rules (see [`fmt_g`]). The field-width / justification prefixes in
/// the Pascal specs (`%10.6g`, `%-13.11g`, …) only space-pad, which the
/// comparator trims, so they are not reproduced here.
pub fn g(v: f64, sig: usize) -> String {
    fmt_g(v, sig)
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
pub fn fixed_w(v: f64, width: usize, decimals: usize) -> String {
    format!("{v:>width$.decimals$}")
}

/// Pascal `Format('%Wd', [v])`: integer, right-justified (space-padded) in field
/// width `width`.
pub fn fixed_w_int(v: i64, width: usize) -> String {
    format!("{v:>width$}")
}

/// Pascal `Format('%W.Pg', [v])`: `%g` with `sig` significant digits, right-
/// justified in field width `width`.
pub fn g_w(v: f64, width: usize, sig: usize) -> String {
    let s = fmt_g(v, sig);
    format!("{s:>width$}")
}

/// Pascal `Format('%-W.Pg', [v])`: `%g` with `sig` significant digits,
/// **left**-justified in field width `width` (the `ShowBuses` coordinate columns).
pub fn g_left_w(v: f64, width: usize, sig: usize) -> String {
    let s = fmt_g(v, sig);
    format!("{s:<width$}")
}

/// Pascal FPC `Str(v: width)` (the default real-to-string with *only* a field
/// width): scientific notation with `width - 8` fraction digits and a signed,
/// ≥3-digit zero-padded exponent, the whole value right-justified in `width`
/// (a leading space carries the positive-sign slot — `6.639353E+004` →
/// ` 6.639353E+004` at width 14). The convergence report's `|V|`/`Vbase`
/// columns write `VmagSaved: 14` / `NodeVbase: 14` (`Solution.pas`
/// `WriteConvergenceReport`). Probe-matched to the pinned oracle's bytes; the
/// value comparison in the golden is by parsed value, so any last-digit rounding
/// difference (FPC-vs-Rust `{:E}`) is absorbed by the column's printing-floor tol.
pub fn fpc_sci_w(v: f64, width: usize) -> String {
    let frac = width.saturating_sub(8);
    // Rust `{:.*E}` → `6.639353E4` / `-4.2E-3`; reformat the exponent to a sign
    // plus an at-least-3-digit zero-padded magnitude.
    let s = format!("{v:.frac$E}");
    let (mant, exp) = s.split_once('E').unwrap_or((s.as_str(), "0"));
    let (esign, edig) = match exp.strip_prefix('-') {
        Some(r) => ('-', r),
        None => ('+', exp.trim_start_matches('+')),
    };
    let body = format!("{mant}E{esign}{edig:0>3}");
    format!("{body:>width$}")
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
