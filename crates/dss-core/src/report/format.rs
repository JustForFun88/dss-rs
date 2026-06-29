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
