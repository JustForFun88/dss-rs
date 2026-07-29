//! Small string/format helpers from Pascal `Common/Utilities.pas`, ported
//! on demand (only what the engine paths implemented so far actually use).

use crate::obj::base::MmfKind;
use dss_parser::{Parser, ParserError, ParserVars};
use num_complex::Complex64;

// --- Constants from DSSGlobals.pas ---

/// Pascal `EPSILON` — "default tiny floating point".
pub const EPSILON: f64 = 1.0e-12;
/// Pascal `EPSILON2` — "default for real number mismatch testing".
pub const EPSILON2: f64 = 1.0e-3;
/// Pascal `SQRT3` (computed `Sqrt(3.0)` at startup, full precision).
pub fn sqrt3() -> f64 {
    3.0_f64.sqrt()
}
/// Pascal `InvSQRT3x1000` = `1000/Sqrt(3)` (computed at startup).
pub fn inv_sqrt3_x1000() -> f64 {
    1000.0 / 3.0_f64.sqrt()
}
/// TODO(compat): Pascal `CALPHA = (-0.5, -0.866025)` — a deliberately
/// low-precision −120° phasor (DSSGlobals.pas even carries a TODO about it).
/// Used by the Vsource asymmetric-matrix path; the clean fix is the exact
/// `1∠−120°`, and it is a re-baseline rather than a lane flip.
///
/// **Stage F.3z measured it** instead of filing it under "truncated constants".
/// The imaginary part is **4.37e-7** relative short of `−sin 120° =
/// −0.8660254037844386` — only ~2x below the 1e-6-class oracle floors before
/// the solve amplifies it, which is exactly why this one does not survive:
/// with the exact value selected (here *and* at the `Reactor` twin), **33 of
/// the 520** gated corpus cases fail plus the `dump_reactor_symcomp` byte
/// golden. Whole-artifact cost on a third of a hundred decks, so it escapes
/// with that number; the flip belongs to an UPGRADE-rung re-capture.
pub const CALPHA: Complex64 = Complex64::new(-0.5, -0.866025);
/// Pascal `CDOUBLEONE` (DSSUcomplex.pas).
pub const CDOUBLEONE: Complex64 = Complex64::new(1.0, 1.0);

/// Pascal `QuadSolver` (Utilities.pas): the most positive root of
/// `a·x² + b·x + c`, 0.0 by default (incl. `a = b = 0`); `a = 0` solves the
/// linear case. A negative discriminant produces NaN exactly like the
/// unchecked Pascal `sqrt`.
pub fn quad_solver(a: f64, b: f64, c: f64) -> f64 {
    if a == 0.0 {
        if b != 0.0 { -c / b } else { 0.0 }
    } else {
        let mid_term = (b * b - 4.0 * a * c).sqrt();
        let a2 = 2.0 * a;
        let ans1 = (-b + mid_term) / a2;
        let ans2 = (-b - mid_term) / a2;
        if ans1 > ans2 { ans1 } else { ans2 }
    }
}

/// Pascal `StrYOrN`.
pub fn str_y_or_n(b: bool) -> &'static str {
    if b { "Yes" } else { "No" }
}

/// Pascal `CompareTextShortest` = 0: case-insensitive equality over the
/// first `min(len1, len2)` bytes. Note an empty string matches anything,
/// exactly like the original (`Copy(S, 1, 0) = ''`).
pub fn compare_text_shortest_eq(s1: &str, s2: &str) -> bool {
    let (a, b) = (s1.as_bytes(), s2.as_bytes());
    let n = a.len().min(b.len());
    a[..n].eq_ignore_ascii_case(&b[..n])
}

// --- VCL TColor palette (FPC `Graphics`/`GraphType` build unit) ---
//
// These `clXXX` integer constants are NOT in the vendored dss_capi source (they
// come from an FPC build unit), so the values are the standard VCL palette and
// are pinned empirically against the oracle by the plot-callback golden capture
// (`tools/golden/gen_plot_callback.py`; the four exercised by the default payload
// — Blue/Green/Red/Black — plus `$`-hex are confirmed live). TColor is
// `$00BBGGRR`: the low byte is red, the mid byte green, the high byte blue.
const CL_BLACK: i32 = 0x000000;
const CL_MAROON: i32 = 0x000080;
const CL_GREEN: i32 = 0x008000;
const CL_OLIVE: i32 = 0x008080;
const CL_NAVY: i32 = 0x800000;
const CL_PURPLE: i32 = 0x800080;
const CL_TEAL: i32 = 0x808000;
const CL_GRAY: i32 = 0x808080;
const CL_SILVER: i32 = 0x00C0_C0C0;
const CL_RED: i32 = 0x0000FF;
const CL_LIME: i32 = 0x00FF00;
const CL_YELLOW: i32 = 0x00FFFF;
const CL_BLUE: i32 = 0x00FF_0000;
const CL_FUCHSIA: i32 = 0x00FF_00FF;
const CL_AQUA: i32 = 0x00FF_FF00;
const CL_LT_GRAY: i32 = CL_SILVER; // FPC clLtGray = clSilver
const CL_DK_GRAY: i32 = CL_GRAY; // FPC clDkGray = clGray
const CL_WHITE: i32 = 0x00FF_FFFF;

/// The default plot color (`InterpretColorName` starts `Result := clBlue`).
pub const CL_BLUE_DEFAULT: i32 = CL_BLUE;

/// Pascal `Utilities.InterpretColorName` (Utilities.pas:1727): the
/// `CompareTextShortest` name table (order-sensitive — the FIRST matching prefix
/// wins), else `StrToInt(S)` (decimal or `$`-hex). Returns `None` when neither a
/// name nor an integer parses — the caller then emits error #724 and falls back
/// to `clBlue` (Pascal keeps `Result := clBlue` from the top and does
/// `DoSimpleMsg 724` in the `except`). The `clXXX` values are pinned by the
/// golden (see the palette note above), not fabricated as authoritative.
pub fn interpret_color_name(s: &str) -> Option<i32> {
    // Same order as the Pascal if-else chain (order matters: a short input like
    // "b" matches the FIRST prefix, `black`, not `blue`).
    const TABLE: &[(&str, i32)] = &[
        ("black", CL_BLACK),
        ("Maroon", CL_MAROON),
        ("Green", CL_GREEN),
        ("Olive", CL_OLIVE),
        ("Navy", CL_NAVY),
        ("Purple", CL_PURPLE),
        ("Teal", CL_TEAL),
        ("Gray", CL_GRAY),
        ("Silver", CL_SILVER),
        ("Red", CL_RED),
        ("Lime", CL_LIME),
        ("Yellow", CL_YELLOW),
        ("Blue", CL_BLUE),
        ("Fuchsia", CL_FUCHSIA),
        ("Aqua", CL_AQUA),
        ("LtGray", CL_LT_GRAY),
        ("DkGray", CL_DK_GRAY),
        ("White", CL_WHITE),
    ];
    for (name, color) in TABLE {
        if compare_text_shortest_eq(name, s) {
            return Some(*color);
        }
    }
    // Pascal `StrToInt(S)`: decimal, or a `$`-prefixed hex literal (the corpus
    // form, e.g. `C1=$00FF00FF`). FPC also accepts `0x`; other radix prefixes are
    // not part of the color contract and are left to the decimal/hex parse.
    str_to_int(s)
}

/// Pascal RTL `StrToInt`, the subset the color parser needs: a plain decimal
/// integer or a `$`/`0x`-prefixed hexadecimal literal.
fn str_to_int(s: &str) -> Option<i32> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix('$') {
        return u32::from_str_radix(hex, 16).ok().map(|v| v as i32);
    }
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        return u32::from_str_radix(hex, 16).ok().map(|v| v as i32);
    }
    t.parse::<i32>().ok()
}

/// Pascal `PlotOptions.ColorToHTML` (PlotOptions.pas:172): a TColor (`$00BBGGRR`)
/// rendered `#RRGGBB` — `IntToHex(c and clRed, 2)` (red, low byte), then green
/// (`and clLime shr 8`), then blue (`and clBlue shr 16`), each 2 uppercase hex
/// digits.
pub fn color_to_html(c: i32) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        c & 0xFF,
        (c >> 8) & 0xFF,
        (c >> 16) & 0xFF
    )
}

/// Pascal `InterpretYesNo`: looks only at the first character —
/// `y`/`t` → true, anything else → false. (The original reads `S[1]`
/// unconditionally; an empty string is undefined behavior there and simply
/// false here.)
pub fn interpret_yes_no(s: &str) -> bool {
    matches!(
        s.as_bytes().first().map(u8::to_ascii_lowercase),
        Some(b'y') | Some(b't')
    )
}

/// Pascal `ParseObjectClassandName`: split `class.objname` at the first dot
/// (no dot → all object name, empty class) and run the object name through
/// the parser's `@variable` substitution.
pub fn parse_object_class_and_name(
    parser: &mut Parser,
    vars: &ParserVars,
    full_obj_name: &str,
) -> (String, String) {
    let (class_name, mut obj_name) = match full_obj_name.find('.') {
        None => (String::new(), full_obj_name.to_string()),
        Some(dot) => (
            full_obj_name[..dot].to_string(),
            full_obj_name[dot + 1..].to_string(),
        ),
    };
    parser.check_for_var_in(vars, &mut obj_name);
    (class_name, obj_name)
}

/// Pascal `CheckForBlanks`: quote a string containing spaces unless it
/// already starts with a quote/bracket character.
pub fn check_for_blanks(s: &str) -> String {
    if s.contains(' ')
        && !matches!(
            s.as_bytes().first(),
            Some(b'(' | b'[' | b'{' | b'"' | b'\'')
        )
    {
        format!("\"{s}\"")
    } else {
        s.to_string()
    }
}

/// FPC `FloatToStr` stand-in. Rust's shortest round-trip formatting differs
/// from FPC in digit count and never uses scientific notation, so golden
/// comparisons must always parse numbers out instead of diffing strings
/// (PORTING_PLAN.md §4 tolerance policy).
pub fn float_to_str(v: f64) -> String {
    // FPC `FloatToStr(Double)` = `FloatToStrF(v, ffGeneral, 15, 0)`: 15 significant
    // digits, general (fixed-or-scientific) notation, trailing zeros stripped —
    // i.e. C `%.15g` (what [`fmt_g`] with `sig = 15` produces). Rust's `{}` emits
    // the *shortest round-tripping* form instead (up to 17 digits), which the
    // numeric-tolerance gates (`props_roundtrip`) never distinguished but the
    // byte-exact `Dump`/`Save` gate does (a 16th digit on e.g. `NormAmps`).
    // The one byte-level difference from C `%g`: FPC's `ffGeneral` emits an
    // **uppercase** `E` exponent, unpadded (probe-confirmed via the VSource
    // `puZIdeal` complex property dump: `1E-6`, not `1e-6`/`1E-06`) — matches
    // [`crate::report::format::g`]'s exponent fixup exactly.
    fmt_g(v, 15).replace('e', "E")
}

/// Pascal `FloatToStrEx` from DSSObjectHelper.pas: NaN prints as `----`
/// (the "ignore this property" marker used by Save/dumps).
pub fn float_to_str_ex(v: f64) -> String {
    if v.is_nan() {
        "----".to_string()
    } else {
        float_to_str(v)
    }
}

/// FPC `TStrings.CommaText` (`GetDelimitedText` with the default `Delimiter=','`
/// / `QuoteChar='"'`, non-strict): join the items with commas, wrapping in
/// double-quotes — and doubling any embedded quote — every item that contains any
/// char `<= ' '` (space/control), the delimiter, or the quote char. Used for the
/// `Export Monitors` header row (`Monitor.pas` `TranslateToCSV` →
/// `Header.CommaText`), where labels like `Tap (pu)` / `S1 (kVA)` / `%kW Stored`
/// carry spaces and are quoted so the header line matches the oracle verbatim.
///
/// The **empty-item** case is out of contract and deliberately not modeled: no
/// monitor header label is ever empty (every branch of `ClearMonitorStream` emits
/// a non-empty name), and FPC's empty-item handling is a quirky accumulator path
/// (`if Result='' then …`, which can even drop the delimiter) that no oracle probe
/// pins here — reproducing it would be a guess, not a verified behavior.
pub fn comma_text(items: &[String]) -> String {
    items
        .iter()
        .map(|s| {
            let needs_quote = s.chars().any(|c| c <= ' ' || c == '"' || c == ',');
            if needs_quote {
                format!("\"{}\"", s.replace('"', "\"\""))
            } else {
                s.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// The **F-FMT seam** entry point for "general" (`%g`) number rendering
/// (`DE_PASCALIZE_PLAN.md` Part IV.2 §F-FMT, step F.4).
///
/// Every engine path that turns an `f64` into general-notation text calls this
/// one function; *which* kernel it lands in is the lane's single choice, made
/// in [`crate::compat::fmt_g`]:
///
/// * parity lane — [`fmt_g_fpc_impl`], the loop-for-loop FPC 3.2.2 RTL
///   pipeline (Grisu1 + `ffGeneral` post-processing), byte-identical to the
///   pinned oracle;
/// * default lane — [`fmt_g_native_impl`], the same *notation policy* rendered
///   with Rust's correctly-rounded `format!`.
///
/// Both kernels are always compiled and are asserted against each other by
/// `crates/dss-core/tests/fmt_battery.rs` over the 13 198-value FPC battery.
#[inline]
pub fn fmt_g(v: f64, sig: usize) -> String {
    crate::compat::fmt_g(v, sig)
}

/// The number of significant digits an FPC `%.<sig>g` actually prints.
///
/// `FloatToStrFIntl` clamps the requested precision to `maxdigits` (= 15
/// without `FPC_HAS_TYPE_EXTENDED`), and Grisu1's `n_digits_sci` — derived from
/// the `Str(v : precision + 7)` width as `width - 1 - 1 - 1 - 1 - 3` — is then
/// floored at 2. Shared by both kernels so the two lanes always print the same
/// *number of digits* (and therefore the same column widths); only the value of
/// the last digit may differ.
fn fpc_general_digits(sig: usize) -> usize {
    sig.clamp(2, 15)
}

/// FPC `Format('%-.Ng', …)` / `FloatToStrF(ffGeneral, sig)`: a faithful port of
/// the FPC 3.2.2 RTL float→string pipeline the pinned oracle runs —
/// `Str(v : sig+7)` = the **Grisu1** `str_real` (`rtl/inc/flt_core.inc`, the
/// default non-`FLOAT_ASCII_FALLBACK` converter, `VALREAL_64` shape), then
/// `FloatToStrFIntl`'s `ffGeneral` post-processing
/// (`rtl/objpas/sysutils/sysstr.inc:1435-1532`, `fvDouble` branch:
/// `Str(Double(Value):precision+7)`), which removes the exponent when
/// `P + Exponent < PE` and `Exponent > -6` (hence the long-observed "fixed
/// notation down to 1e-5" FPC threshold) and strips trailing zeros.
///
/// **The parity kernel of the F-FMT seam** ([`fmt_g`]). Its one deliberate
/// inexactness — the reason the row is lane-split at all — is that the final
/// cut to `sig` digits is **not correctly rounded**: Grisu1 first produces 17
/// ties-to-even digits, then `round_digits` re-rounds that *decimal string* to
/// `sig` digits **half-away-from-zero** (the FPC `GRISU1_F2A_HALF_ROUNDUP` +
/// `GRISU1_F2A_AGRESSIVE_ROUNDUP` build flags), so a value just below a decimal
/// half-boundary can round up where a correctly rounded `%.Ng` rounds down
/// (oracle-probed: `loadshape.default`'s computed `FMean` =
/// 0.82582833333333349745… → 17 digits `…33350` → prints `0.825828333333334`,
/// where correct 15-digit rounding gives `…333`). That is exactly what
/// [`fmt_g_native_impl`] fixes in the default lane; here it is *reproduced*,
/// and pinned bit-exact against the real FPC 3.2.2 RTL by the committed battery
/// `tests/golden/fmt_battery.csv` — 13 198 values × 7 render forms
/// (`crates/dss-core/tests/fmt_battery.rs`; generator: `tools/fpc/fmt_battery/`)
/// — plus the byte-exact Dump/Save goldens.
pub fn fmt_g_fpc_impl(v: f64, sig: usize) -> String {
    // `FloatToStrFIntl`: `If (Precision = -1) Or (Precision > maxdigits) Then
    // Precision := maxdigits` (= 15 without FPC_HAS_TYPE_EXTENDED), then
    // `Str(Double(Value) : precision + 7, Result)`.
    let precision = sig.min(15) as i32;
    let sci = grisu_str_real(precision + 7, v);
    fpc_general_post(sci)
}

/// The **default kernel** of the F-FMT seam ([`fmt_g`]): the same general
/// notation, rendered through Rust's correctly-rounded `format!`.
///
/// What it keeps from FPC, deliberately, and why: the *digit count*
/// ([`fpc_general_digits`]), the *fixed-vs-scientific window* (`-6 < exp <
/// digits` — one decade wider than C's `%g`, which stops at `-4`), trailing-zero
/// stripping and the uppercase `E` exponent with no `+` and no zero padding.
/// None of those is an inexactness: they are the layout policy the fixed-width
/// `Show` tables and the `Export` column widths are built around
/// (`DE_PASCALIZE_PLAN.md` IV.1 keeps report *structure* contractual), so
/// changing them would move columns, not precision. What it drops is the one
/// thing IV.2 §F-FMT calls a wart — FPC's two-stage decimal re-rounding: here
/// every digit comes from a single correctly-rounded conversion, so
/// `0.82582833333333349745…` prints `0.825828333333333` at 15 digits.
///
/// The two kernels therefore agree except where the FPC round-up rules bite;
/// `fmt_battery.rs` measures that population over the committed FPC battery.
pub fn fmt_g_native_impl(v: f64, sig: usize) -> String {
    if !v.is_finite() {
        // `str_real`'s `return_special` (GRISU1_F2A_NAN_SIGNLESS), post-processed
        // by `fpc_general_post`'s "no `.` → return as-is" early exit. Not a
        // number, so correct rounding has nothing to say about it: both kernels
        // spell it the same way.
        return if v.is_nan() {
            "Nan".to_string()
        } else if v.is_sign_negative() {
            "-Inf".to_string()
        } else {
            "+Inf".to_string()
        };
    }

    let digits = fpc_general_digits(sig);
    // One correctly-rounded conversion supplies both the decimal exponent and
    // the digits; `{:.*e}` is exact in Rust (no double rounding).
    let sci = format!("{:.*e}", digits - 1, v);
    let (mantissa, exp) = sci.split_once('e').expect("scientific form has an `e`");
    let exp: i32 = exp.parse().expect("exponent parses as i32");

    let mut out = if exp > -6 && exp < digits as i32 {
        // Fixed notation: `digits` significant figures means `digits-1-exp`
        // fractional places. Re-rendering from `v` (rather than shifting the
        // mantissa string) keeps the single-rounding property, and the two
        // roundings agree because `exp` already reflects any carry out of the
        // leading digit (9.999 → 1.000e1).
        let decimals = (digits as i32 - 1 - exp).max(0) as usize;
        strip_trailing_zeros(format!("{v:.decimals$}"))
    } else {
        // Scientific: FPC drops the `+` and the exponent's leading zeros.
        format!("{}E{exp}", strip_trailing_zeros(mantissa.to_string()))
    };

    // `RemoveLeadingNegativeSign`: a value that rounded to zero prints `0`,
    // never `-0` (same character set as the parity kernel's guard).
    if out.len() > 1
        && out.starts_with('-')
        && out[1..]
            .bytes()
            .all(|c| matches!(c, b'0' | b'.' | b'E' | b'+' | b','))
    {
        out.remove(0);
    }
    out
}

/// Drop a decimal string's trailing zeros and any dangling decimal point
/// (`1.500` → `1.5`, `2.000` → `2`); an integer-shaped string is untouched.
fn strip_trailing_zeros(s: String) -> String {
    if !s.contains('.') {
        return s;
    }
    let t = s.trim_end_matches('0');
    let t = t.strip_suffix('.').unwrap_or(t);
    t.to_string()
}

// =========================================================================
// FPC 3.2.2 Grisu1 float→ASCII (`rtl/inc/flt_core.inc`, VALREAL_64 = double,
// hardfloat build): the `str_real` behind `Str(v:w)` / `FloatToStrF` /
// `Format('%g')`. Ported loop-for-loop; only the exponential output path is
// reachable here (`Str(v:w)` passes `frac_digits < 0`).
// =========================================================================

/// `TDIY_FP` (64-bit shape): `f · 2^e`.
#[derive(Clone, Copy)]
struct DiyFp {
    f: u64,
    e: i32,
}

/// `diy_fp_multiply` (`flt_core.inc:261-288`, VALREAL_64): the rounded
/// high-64 of the 128-bit product — the 32×32 partial-sum formula is exactly
/// `(x·y + 2^63) >> 64` — with `e := x.e + y.e + 64` and optional
/// re-normalization.
fn diy_fp_multiply(x: DiyFp, y: DiyFp, normalize: bool) -> DiyFp {
    let prod = u128::from(x.f) * u128::from(y.f);
    let f = ((prod + (1u128 << 63)) >> 64) as u64;
    let mut r = DiyFp {
        f,
        e: x.e + y.e + 64,
    };
    if normalize && r.f >> 63 == 0 {
        r.f += r.f; // `inc(f, f)` — {$Q-} wrapping add
        r.e -= 1;
    }
    r
}

/// `diy_fp_cached_power10` (`flt_core.inc:432-678`, VALREAL_64 tables):
/// normalized power-of-10 factor `c · 2^e = 10^e10` nearest above `exp10`,
/// composed from the sparse base cache × plus/minus factors + the per-index
/// ulp corrector.
fn diy_fp_cached_power10(exp10: i32) -> (DiyFp, i32) {
    // alpha = -61; gamma = 0; full cache 1E-450..1E+432, step 1E+18, sparse 1/10.
    const C_PWR10_DELTA: i32 = 18;
    const C_PWR10_COUNT: i32 = 50;
    const BASE: [(u64, i32, i32); 10] = [
        (0x825E_CC24_C873_7830, -362, -90),
        (0xE228_0B6C_20DD_5232, -303, -72),
        (0xC428_D05A_A475_1E4D, -243, -54),
        (0xAA24_2499_6973_92D3, -183, -36),
        (0x9392_EE8E_921D_5D07, -123, -18),
        (0x8000_0000_0000_0000, -63, 0),
        (0xDE0B_6B3A_7640_0000, -4, 18),
        (0xC097_CE7B_C907_15B3, 56, 36),
        (0xA70C_3C40_A64E_6C52, 116, 54),
        (0x90E4_0FBE_EA1D_3A4B, 176, 72),
    ];
    const FACTOR_PLUS: [(u64, i32, i32); 2] = [
        (0xF6C6_9A72_A398_9F5C, 534, 180),
        (0xEDE2_4AE7_98EC_8284, 1132, 360),
    ];
    const FACTOR_MINUS: [(u64, i32, i32); 2] = [
        (0x84C8_D4DF_D2C6_3F3B, -661, -180),
        (0x89BF_7228_4032_7F82, -1259, -360),
    ];
    const CORRECTOR: [i8; C_PWR10_COUNT as usize] = [
        0, 0, 0, 0, 1, 0, 0, 0, 1, -1, 0, 1, 1, 1, -1, 0, 0, 1, 0, -1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, -1, 0, 0, -1, 0, 0, 0, 0, 0, -1, 0, 0, 0, 0, 1, 0, 0, 0, -1, 0,
    ];

    let min10 = BASE[0].2 + FACTOR_MINUS[1].2; // -450
    let i = if exp10 <= min10 {
        0
    } else {
        let mut i = (exp10 - min10) / C_PWR10_DELTA;
        if i * C_PWR10_DELTA + min10 != exp10 {
            i += 1; // round-up
        }
        i.min(C_PWR10_COUNT - 1)
    };
    let inod = (i % 10) as usize;
    let xmul = i / 10 - FACTOR_MINUS.len() as i32;
    let a = DiyFp {
        f: BASE[inod].0,
        e: BASE[inod].1,
    };
    let a_e10 = BASE[inod].2;
    if xmul == 0 {
        return (a, a_e10);
    }
    let (fac, fac_e10) = if xmul > 0 {
        let x = (xmul - 1) as usize;
        (
            DiyFp {
                f: FACTOR_PLUS[x].0,
                e: FACTOR_PLUS[x].1,
            },
            FACTOR_PLUS[x].2,
        )
    } else {
        let x = (-(xmul + 1)) as usize;
        (
            DiyFp {
                f: FACTOR_MINUS[x].0,
                e: FACTOR_MINUS[x].1,
            },
            FACTOR_MINUS[x].2,
        )
    };
    let e10 = a_e10 + fac_e10;
    if a_e10 == 0 {
        return (fac, e10); // exact — no corrector
    }
    let mut c = diy_fp_multiply(a, fac, true);
    let cx = CORRECTOR[i as usize];
    if cx != 0 {
        c.f = c.f.wrapping_add(cx as i64 as u64); // `inc(factor.c.f, int64(cx))`
    }
    (c, e10)
}

/// `k_comp` (`flt_core.inc:1504-1541`, hardfloat): the exp10 of the factor
/// bringing `e` into `[alpha .. gamma]` — `ceil((alpha - e) · log10 2)`
/// computed in plain double arithmetic.
fn grisu_k_comp(e: i32, alpha: i32) -> i32 {
    // FPC's literal `0.301029995663981195213738894724493027` parses to the
    // same f64 bits as the std constant (0x1.34413509f79ffp-2).
    const D_LOG10_2: f64 = std::f64::consts::LOG10_2;
    let x = alpha - e;
    let dexp = f64::from(x) * D_LOG10_2;
    let mut n = dexp.trunc() as i32;
    if x > 0 && dexp != f64::from(n) {
        n += 1; // round-up
    }
    n
}

/// `gen_digits_32` (`flt_core.inc:758-803`): decimal digits of a `u32` into
/// `buf[pos..]`, optionally zero-padded to 9.
fn gen_digits_32(buf: &mut [u8], pos: usize, x: u32, pad_9zero: bool) -> usize {
    const DIGITS: [u32; 10] = [
        0, 10, 100, 1000, 10000, 100000, 1000000, 10000000, 100000000, 1000000000,
    ];
    let mut n = if x == 0 {
        0
    } else {
        let mut n = (((31 - x.leading_zeros()) + 1) * 1233) >> 12;
        if x >= DIGITS[n as usize] {
            n += 1;
        }
        n as usize
    };
    if pad_9zero && n < 9 {
        n = 9;
    }
    let count = n;
    let mut m = x;
    while n > 0 {
        n -= 1;
        if m != 0 {
            let z = m / 10;
            buf[pos + n] = (m - z * 10) as u8;
            m = z;
        } else {
            buf[pos + n] = 0;
        }
    }
    count
}

/// `gen_digits_64` (`flt_core.inc:806-845`): decimal digits of a `u64` via a
/// 3-way split at 1e9.
fn gen_digits_64(buf: &mut [u8], pos: usize, x: u64, pad_19zero: bool) -> usize {
    let (splith, splitm, splitl): (u32, u32, u32);
    if x < 1_000_000_000 {
        splith = 0;
        splitm = 0;
        splitl = x as u32;
    } else {
        let temp = x / 1_000_000_000;
        splitl = (x - temp * 1_000_000_000) as u32;
        if temp < 1_000_000_000 {
            splith = 0;
            splitm = temp as u32;
        } else {
            splith = (temp / 1_000_000_000) as u32;
            splitm = (temp as u32).wrapping_sub(splith.wrapping_mul(1_000_000_000));
        }
    }
    let mut n_digits = gen_digits_32(buf, pos, splith, false);
    if pad_19zero && n_digits == 0 {
        buf[pos] = 0;
        n_digits = 1;
    }
    n_digits += gen_digits_32(buf, pos + n_digits, splitm, n_digits != 0);
    n_digits += gen_digits_32(buf, pos + n_digits, splitl, n_digits != 0);
    n_digits
}

/// `round_digits` (`flt_core.inc:854-926`): round the digit sequence down to
/// `n_max` digits; returns the decimal-point correction (1 on overflow out of
/// the first digit). `half_round_to_even = false` is the FPC
/// `GRISU1_F2A_HALF_ROUNDUP` mode with the `AGRESSIVE_ROUNDUP` hack.
fn round_digits(
    buf: &mut [u8],
    n_current: &mut usize,
    n_max: usize,
    half_round_to_even: bool,
) -> i32 {
    let n = *n_current;
    *n_current = n_max;
    let mut dig_round = buf[n_max];

    // GRISU1_F2A_AGRESSIVE_ROUNDUP: a "4" followed by only "9"s up to a
    // large-ish second-last digit is treated as ">= 5".
    if !half_round_to_even && dig_round == 4 && n_max < n - 3 && buf[n - 2] >= 8 {
        let mut i = n - 2;
        loop {
            i -= 1;
            if i == n_max || buf[i] != 9 {
                break;
            }
        }
        if i == n_max {
            dig_round = 9; // force round-up
        }
    }

    if dig_round < 5 {
        return 0;
    }
    // "Round half to even": exactly-half checks the sticky tail.
    if dig_round == 5 && half_round_to_even && (n_max == 0 || buf[n_max - 1] & 1 == 0) {
        let mut dig_sticky = 0u8;
        let mut n = n;
        while n > n_max + 1 && dig_sticky == 0 {
            n -= 1;
            dig_sticky = buf[n];
        }
        if dig_sticky == 0 {
            return 0; // exactly a half -> no rounding
        }
    }
    // Round-up.
    let mut n_max = n_max;
    while n_max > 0 {
        n_max -= 1;
        buf[n_max] += 1;
        if buf[n_max] < 10 {
            *n_current = n_max + 1;
            return 0;
        }
    }
    // Overflow out of the 1st digit.
    buf[0] = 1;
    *n_current = 1;
    1
}

/// `return_exponential` (`flt_core.inc:1130-1228`): ` d.ddd…E±xxx` with a
/// space sign-slot, `n_digits_req` mantissa digits (zero-padded) and an
/// `n_digits_exp`-zero-padded exponent, space-padded to `min_width`.
fn return_exponential(
    minus: bool,
    digits: &[u8],
    n_digits_have: usize,
    n_digits_req: usize,
    d_exp: i32,
    n_digits_exp: usize,
    min_width: i32,
) -> String {
    let e_minus = d_exp < 0;
    let d_exp = d_exp.unsigned_abs();
    let mut buf_exp = [0u8; 40];
    let n_exp = gen_digits_32(&mut buf_exp, 0, d_exp, false);
    let mut s = String::new();
    // Leading spaces (computed after assembling the body — same result).
    let mut body = String::new();
    body.push(if minus { '-' } else { ' ' });
    body.push(if n_digits_have > 0 {
        char::from(digits[0] + b'0')
    } else {
        '0'
    });
    if n_digits_req > 1 {
        body.push('.');
    }
    let mut j = 1usize;
    while j < n_digits_have && j < n_digits_req {
        body.push(char::from(digits[j] + b'0'));
        j += 1;
    }
    for _ in j..n_digits_req {
        body.push('0');
    }
    body.push('E');
    body.push(if e_minus { '-' } else { '+' });
    for _ in n_exp..n_digits_exp {
        body.push('0');
    }
    for &d in buf_exp.iter().take(n_exp) {
        body.push(char::from(d + b'0'));
    }
    let n_spaces = min_width - body.len() as i32;
    for _ in 0..n_spaces.max(0) {
        s.push(' ');
    }
    s.push_str(&body);
    s
}

/// Grisu1 `str_real(min_width, frac_digits, v, RT_S64REAL, str)`
/// (`flt_core.inc:687-1902`, VALREAL_64) for the `Str(v : min_width)` call
/// shape (`frac_digits < 0` → the exponential path; the fixed path is
/// unreachable from `%g`/`FloatToStr` and not ported).
fn grisu_str_real(min_width_in: i32, v: f64) -> String {
    const C_FRAC2_BITS: i32 = 52;
    const C_EXP2_BIAS: i32 = 1023;
    const C_GRISU_ALPHA: i32 = -61;
    const C_GRISU_GAMMA: i32 = 0;
    const C_EXP2_SPECIAL: i32 = C_EXP2_BIAS * 2 + 1; // 2047
    const C_MANT2_INTEGER: u64 = 1u64 << C_FRAC2_BITS;
    // double profile: 17 mantissa digits, 3 exponent digits.
    const N_DIG_MANTISSA: usize = 17;
    const N_DIG_EXP10: usize = 3;

    let mut min_width = min_width_in;
    if min_width < 0 {
        min_width = 0;
    }

    // n_digits_need = n_digits_req = 17 for double; mantissa digits printed
    // in exponential notation, derived from the width.
    let n_digits_req = N_DIG_MANTISSA;
    let n_digits_need = N_DIG_MANTISSA;
    let n_digits_sci = {
        let n = min_width as isize - 1 - 1 - 1 - 1 - N_DIG_EXP10 as isize;
        n.clamp(2, n_digits_req as isize) as usize
    };

    // Float -> DIY_FP (`unpack_float`).
    let bits = v.to_bits();
    let minus = (bits >> 63) != 0;
    let mut w = DiyFp {
        f: bits & ((1u64 << 52) - 1),
        e: ((bits >> 52) & 0x7ff) as i32,
    };

    let mut buf = [0u8; 40];

    // Zero.
    if w.e == 0 && w.f == 0 {
        return return_exponential(minus, &buf, 0, n_digits_sci, 0, N_DIG_EXP10, min_width);
    }
    // Specials (`return_special`): sign slot then 'Inf'/'Nan', NaN signless.
    if w.e == C_EXP2_SPECIAL {
        let (sign, spec) = if w.f == 0 {
            (if minus { "-" } else { "+" }, "Inf")
        } else {
            ("", "Nan") // GRISU1_F2A_NAN_SIGNLESS
        };
        let mw = if min_width_in < 0 {
            (N_DIG_MANTISSA + N_DIG_EXP10 + 4) as i32
        } else {
            min_width
        };
        let body = format!("{sign}{spec}");
        let n_spaces = mw - body.len() as i32;
        return format!("{}{}", " ".repeat(n_spaces.max(0) as usize), body);
    }
    // Normal / denormal.
    let n;
    if w.e != 0 {
        w.f |= C_MANT2_INTEGER;
        n = 64 - C_FRAC2_BITS - 1; // C_DIY_FP_Q - C_FRAC2_BITS - 1 = 11
    } else {
        n = w.f.leading_zeros() as i32;
        w.e += 1;
    }
    w.f <<= n;
    w.e -= C_EXP2_BIAS + n + C_FRAC2_BITS;

    // Scale into [alpha .. gamma].
    let (d_fp, c_mk_e10) = if (C_GRISU_ALPHA..=C_GRISU_GAMMA).contains(&w.e) {
        (w, 0)
    } else {
        let mk = grisu_k_comp(w.e, C_GRISU_ALPHA);
        let (c, e10) = diy_fp_cached_power10(mk);
        if e10 == 0 {
            (w, e10)
        } else {
            (diy_fp_multiply(w, c, false), e10)
        }
    };

    // Integer part digits.
    let mut n_digits_have = gen_digits_64(&mut buf, 0, d_fp.f >> (-d_fp.e), false);
    let mut dot_pos = n_digits_have as i32;

    // Fractional part digits ({$Q-} wrapping arithmetic).
    let mut f: u32 = 0;
    if d_fp.e < 0 {
        let mut one_e = d_fp.e;
        let mut one_maskl: u64 = (1u64 << (-d_fp.e)) - 1;
        let mut fl: u64 = d_fp.f & one_maskl;
        // 64-bit loop.
        while one_e < -29 && n_digits_have < n_digits_need + 1 && fl != 0 {
            fl = fl.wrapping_add(fl << 2); // f := f * 5
            one_maskl >>= 1; // one := one / 2
            one_e += 1;
            buf[n_digits_have] = (fl >> (-one_e)) as u8;
            fl &= one_maskl;
            n_digits_have += 1;
        }
        // Pascal `n_digits_have >= n_digits_need + 1`.
        if n_digits_have > n_digits_need {
            f = u32::from(fl != 0); // only "sticky" remains
        } else {
            let mut one_mask: u32 = one_maskl as u32;
            f = fl as u32;
            // 32-bit loop.
            while n_digits_have < n_digits_need + 1 && f != 0 {
                f = f.wrapping_add(f << 2);
                one_mask >>= 1;
                one_e += 1;
                buf[n_digits_have] = (f >> (-one_e)) as u8;
                f &= one_mask;
                n_digits_have += 1;
            }
        }
    }
    // Append the "sticky" digit if any (Pascal `>= n_digits_need + 1`).
    if f != 0 && n_digits_have > n_digits_need {
        n_digits_have = n_digits_need + 2;
        buf[n_digits_need + 1] = 1;
    }
    // Round to 17 digits, ties-to-even.
    if n_digits_have > n_digits_need {
        dot_pos += round_digits(&mut buf, &mut n_digits_have, n_digits_need, true);
    }
    // (frac_digits < 0 → no fixed-notation attempt.)
    // Round to the width-derived digit count, HALF_ROUNDUP + aggressive.
    if n_digits_have > n_digits_sci {
        dot_pos += round_digits(&mut buf, &mut n_digits_have, n_digits_sci, false);
    }
    return_exponential(
        minus,
        &buf,
        n_digits_have,
        n_digits_sci,
        dot_pos - c_mk_e10 - 1,
        N_DIG_EXP10,
        min_width,
    )
}

/// FPC `FloatToStrFIntl` `ffGeneral` post-processing over the `Str` scientific
/// output (`sysstr.inc:1435-1532`): drop the exponent when the value fits
/// fixed notation (`P + Exponent < PE` and `Exponent > -6`), strip trailing
/// zeros / dangling point / superfluous exponent characters, and remove a
/// leading `-` that would print as negative zero.
fn fpc_general_post(sci: String) -> String {
    let mut r: Vec<u8> = sci.into_bytes();
    while r.first() == Some(&b' ') {
        r.remove(0);
    }
    let Some(p0) = r.iter().position(|&c| c == b'.') else {
        return String::from_utf8(r).expect("ASCII"); // NAN or other special
    };
    if let Some(pe0) = r.iter().position(|&c| c == b'E') {
        // Read the exponent.
        let mut exponent: i32 = 0;
        for &c in &r[pe0 + 2..] {
            exponent = exponent * 10 + i32::from(c - b'0');
        }
        if r[pe0 + 1] == b'-' {
            exponent = -exponent;
        }
        if (p0 as i32) + exponent < pe0 as i32 && exponent > -6 {
            // OK to remove the exponent → fixed notation.
            r.truncate(pe0);
            let mut p = p0;
            if exponent >= 0 {
                // Shift the point right.
                for _ in 0..exponent {
                    r[p] = r[p + 1];
                    p += 1;
                }
                r[p] = b'.';
                let mut q = 0usize;
                if r[q] == b'-' {
                    q += 1;
                }
                // Trim leading zeros (rounding can produce one).
                while r[q] == b'0' && q < r.len() - 1 && r[q + 1] != b'.' {
                    r.remove(q);
                }
            } else {
                // Add zeros at the start: `Insert(Copy('00000',1,-Exponent),
                // Result, P-1)` then re-place the leading digit and the point.
                let e = (-exponent) as usize;
                for _ in 0..e {
                    r.insert(p - 1, b'0');
                }
                r[p + e] = r[p + e - 1];
                r[p] = b'.';
                if exponent != -1 {
                    r[p + e - 1] = b'0';
                }
            }
            // Remove trailing zeros / dangling point (1-based Q walk).
            let mut q = r.len();
            while q > 0 && r[q - 1] == b'0' {
                q -= 1;
            }
            if q > 0 && r[q - 1] == b'.' {
                q -= 1;
            }
            if q == 0 || (q == 1 && r[0] == b'-') {
                r = b"0".to_vec();
            } else {
                r.truncate(q);
            }
        } else {
            // Keep the exponent; remove superfluous characters.
            let mut pe = pe0;
            while r[pe - 1] == b'0' {
                r.remove(pe - 1);
                pe -= 1;
            }
            if r[pe - 1] == b'.' {
                r.remove(pe - 1);
                pe -= 1;
            }
            if r[pe + 1] == b'+' {
                r.remove(pe + 1);
            } else {
                pe += 1;
            }
            while pe + 1 < r.len() && r[pe + 1] == b'0' {
                r.remove(pe + 1);
            }
        }
    }
    // `RemoveLeadingNegativeSign`: "-0"/"-0.00…" → drop the sign.
    if r.len() > 1 && r[0] == b'-' {
        let all_zeroish = r[1..]
            .iter()
            .all(|&c| matches!(c, b'0' | b'.' | b'E' | b'+' | b','));
        if all_zeroish {
            r.remove(0);
        }
    }
    String::from_utf8(r).expect("ASCII")
}

/// Pascal `GetDSSArray` for doubles: `''` for a NIL array, otherwise
/// `[ v1 v2 ...]` with each value divided by `scale`.
pub fn get_dss_array_f64(n: usize, dbls: Option<&[f64]>, scale: f64) -> String {
    let Some(dbls) = dbls else {
        return String::new();
    };
    let mut result = String::from("[");
    // The Pascal indexes 1..n blindly; we stop at the slice end to stay safe
    // (reading past the allocation there is undefined behavior, not a
    // reproducible quirk).
    for v in dbls.iter().take(n) {
        let value = if scale == 1.0 { *v } else { *v / scale };
        result.push(' ');
        result.push_str(&float_to_str(value));
    }
    result.push(']');
    result
}

/// Pascal `GetDSSArray` for integers (`Format(' %-.d', ...)` is just the
/// plain digits).
pub fn get_dss_array_i32(n: usize, ints: Option<&[i32]>) -> String {
    let Some(ints) = ints else {
        return String::new();
    };
    let mut result = String::from("[");
    for v in ints.iter().take(n) {
        result.push(' ');
        result.push_str(&v.to_string());
    }
    result.push(']');
    result
}

/// A recognized file-backed numeric-array directive (Pascal `InterpretDblArray`,
/// `Utilities.pas:461-566`): `file=` plain text (optionally `column=`/`header=`),
/// `dblfile=` a raw little-endian `f64` stream, `sngfile=` a raw little-endian
/// `f32` stream (widened to `f64`). Recognized at property-parse time by
/// [`parse_dbl_array_file_spec`]; the actual file read is deferred to the
/// executive (which has the filesystem + `LastResultFile`), running the grammar
/// with [`read_dbl_array_text`] / [`read_le_f32_array`] / [`read_le_f64_array`].
#[derive(Debug, Clone)]
pub struct DblArrayFileSpec {
    pub kind: MmfKind,
    /// The filename exactly as written (may be the literal `%result%`, which the
    /// executive resolves against `DSS.LastResultFile`).
    pub filename: String,
    /// 1-based comma/space column (`file=` only; `1` otherwise).
    pub column: i32,
    /// Skip one header line (`file=` `header=yes` only).
    pub header: bool,
}

/// Pascal `InterpretDblArray` directive recognizer (`Utilities.pas:454-566`):
/// parse the leading `file=`/`dblfile=`/`sngfile=` token and, for `file=`, the
/// `column=`/`header=` options in either order (`CompareTextShortest`). Returns
/// `None` for a plain numeric list (the caller then parses it with
/// [`interpret_dbl_array`]). Uses a private scratch parser mirroring Pascal's
/// `DSS.AuxParser`.
pub fn parse_dbl_array_file_spec(s: &str) -> Option<DblArrayFileSpec> {
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    parser.set_auto_increment(false);
    parser.set_cmd_string(s);
    let parm_name = parser.next_param(&vars);
    let param = parser.make_string(&vars);

    // Pascal: `file` is an exact (case-insensitive) match; `dblfile`/`sngfile`
    // use shortest-prefix, both guarded by a non-empty parameter name.
    if parm_name.eq_ignore_ascii_case("file") {
        let filename = param;
        let mut column = 1;
        let mut header = false;
        // Options may be in either order (`Utilities.pas:481-491`).
        loop {
            let pn = parser.next_param(&vars);
            let pv = parser.make_string(&vars);
            if pv.is_empty() {
                break;
            }
            if compare_text_shortest_eq(&pn, "column") {
                column = parser.make_integer(&vars).unwrap_or(1);
            }
            if compare_text_shortest_eq(&pn, "header") {
                header = interpret_yes_no(&pv);
            }
        }
        Some(DblArrayFileSpec {
            kind: MmfKind::Text,
            filename,
            column,
            header,
        })
    } else if !parm_name.is_empty() && compare_text_shortest_eq(&parm_name, "dblfile") {
        Some(DblArrayFileSpec {
            kind: MmfKind::Float64,
            filename: param,
            column: 1,
            header: false,
        })
    } else if !parm_name.is_empty() && compare_text_shortest_eq(&parm_name, "sngfile") {
        Some(DblArrayFileSpec {
            kind: MmfKind::Float32,
            filename: param,
            column: 1,
            header: false,
        })
    } else {
        None
    }
}

/// Pascal `InterpretDblArray` `file=` text branch (`Utilities.pas:493-524`):
/// read up to `max` rows from `content`, optionally skipping one `header` line,
/// taking the 1-based comma/space-delimited `column` from each row. Returns the
/// values actually read plus the 1-based row that raised a numeric-conversion
/// error, if any. A file shorter than `max` yields fewer values (the Pascal
/// short-file `Result := i-1`), and the caller shrinks its element count to
/// `.len()`. A malformed numeric token raises in Pascal (`DoSimpleMsg` #705,
/// then `Result := i-1; Break`, `Utilities.pas:515-521`) — the same stop-and-
/// shrink: reading stops at that row, the returned length is `i-1`, and the
/// row index is returned so the caller can emit the #705 diagnostic. Byte-
/// position faithful to Pascal's `(F.Position + 1) < F.Size` read guard.
pub fn read_dbl_array_text(
    content: &str,
    column: i32,
    header: bool,
    max: usize,
) -> (Vec<f64>, Option<usize>) {
    let bytes = content.as_bytes();
    let size = bytes.len();
    let mut pos = 0usize;
    let mut parser = Parser::new();
    parser.set_auto_increment(false);
    let vars = ParserVars::new();

    if header {
        // Pascal `if CSVHeader then FSReadln(F, InputLine)` — skip one line
        // unconditionally (no size guard).
        read_line_advance(bytes, &mut pos);
    }

    let mut out = Vec::new();
    let mut error_row = None;
    for i in 0..max {
        // Pascal reads a row only while `(F.Position + 1) < F.Size`; otherwise it
        // stops (`Result := i - 1; Break`).
        if pos + 1 >= size {
            break;
        }
        let line = read_line_advance(bytes, &mut pos);
        let line_str = String::from_utf8_lossy(line);
        parser.set_cmd_string(&line_str);
        // Advance `column` params (1-based), then take the current token's value.
        for _ in 0..column.max(0) {
            parser.next_param(&vars);
        }
        // A missing column yields an empty token → 0.0 (Pascal `DblValue`). A
        // malformed token raises in Pascal (`DoSimpleMsg` #705, `Result := i-1;
        // Break`, `Utilities.pas:515-521`) — reproduce the stop-and-shrink: record
        // the 1-based row and break so the returned length equals `i-1` and the
        // caller shrinks its count (proven vs the pinned oracle: `mult=(file=…)`
        // with row 3 = `abc` yields npts=2, mult=[0.1,0.2] + a #705 error).
        match parser.make_double(&vars) {
            Ok(v) => out.push(v),
            Err(_) => {
                error_row = Some(i + 1);
                break;
            }
        }
    }
    (out, error_row)
}

/// Read one line from `bytes` starting at `*pos`, advancing `*pos` past the
/// terminating `\n` (or to EOF); the returned slice excludes the `\r\n`/`\n`.
fn read_line_advance<'a>(bytes: &'a [u8], pos: &mut usize) -> &'a [u8] {
    let start = *pos;
    let mut end = start;
    while end < bytes.len() && bytes[end] != b'\n' {
        end += 1;
    }
    let mut line_end = end;
    if line_end > start && bytes[line_end - 1] == b'\r' {
        line_end -= 1;
    }
    *pos = if end < bytes.len() { end + 1 } else { end };
    &bytes[start..line_end]
}

/// Pascal `InterpretDblArray` `sngfile=` branch (`Utilities.pas:545-565`): read
/// `min(max, size/4)` little-endian `f32` records, each widened to `f64` (this
/// is the flat-array path — NOT the LoadShape `ReadSngFile` `UseFloat32` /
/// `(hour,mult)`-pair path).
pub fn read_le_f32_array(bytes: &[u8], max: usize) -> Vec<f64> {
    let k = (bytes.len() / 4).min(max);
    bytes[..k * 4]
        .chunks_exact(4)
        .map(|c| f64::from(f32::from_le_bytes(c.try_into().unwrap())))
        .collect()
}

/// Pascal `InterpretDblArray` `dblfile=` branch (`Utilities.pas:532-543`): read
/// `min(max, size/8)` little-endian `f64` records.
pub fn read_le_f64_array(bytes: &[u8], max: usize) -> Vec<f64> {
    let k = (bytes.len() / 8).min(max);
    bytes[..k * 8]
        .chunks_exact(8)
        .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

/// Pascal `InterpretDblArray`, list-of-numbers path: read exactly
/// `max_values` doubles out of `s` into `out`, filling with zeros when the
/// string runs short (the parser returns 0 for an empty token). The outer
/// `Edit` parser has already stripped the surrounding `()`/`[]`, so `s` is a
/// bare delimiter-separated list; values may themselves be quoted RPN.
///
/// The file-backed spellings (`file=`/`dblfile=`/`sngfile=`) are recognized and
/// deferred *by the caller* ([`parse_dbl_array_file_spec`] in the property
/// engine, before this function is reached). This function keeps the reject as a
/// defensive net for the property paths that Pascal routes through
/// `ParseAsVector` — which do NOT support a file spec (e.g. `DoubleVArrayProperty`
/// `XSCArray`): there a file directive is a genuine input error, not a silent
/// parse of the keyword as 0.
///
/// `out` must be at least `max_values` long. Returns the number of slots
/// written (always `max_values` for the list path, matching the original).
pub fn interpret_dbl_array(
    parser: &mut Parser,
    vars: &ParserVars,
    s: &str,
    max_values: usize,
    out: &mut [f64],
) -> Result<usize, ParserError> {
    parser.set_auto_increment(false);
    parser.set_cmd_string(s);
    let parm_name = parser.next_param(vars);
    let _param = parser.make_string(vars);

    // Pascal: `file` is an exact (case-insensitive) match; `dblfile`/`sngfile`
    // use shortest-prefix. All are guarded by a non-empty parameter name.
    if parm_name.eq_ignore_ascii_case("file")
        || (!parm_name.is_empty()
            && (compare_text_shortest_eq(&parm_name, "dblfile")
                || compare_text_shortest_eq(&parm_name, "sngfile")))
    {
        return Err(ParserError::new(format!(
            "file-backed numeric arrays (\"{parm_name}=\") are not supported yet"
        )));
    }

    for slot in out.iter_mut().take(max_values) {
        // DblValue of the current token (Pascal AutoIncrement is off here),
        // then advance. Empty/exhausted tokens yield 0.0.
        *slot = parser.make_double(vars)?;
        parser.next_param(vars);
    }
    Ok(max_values)
}

/// Read every double present in `s`, with no upper bound — the Pascal
/// `DoubleDArrayProperty` parse path (e.g. an XYcurve `Points`), where the
/// element count is whatever the script supplies. Tokens are read until the
/// parser is exhausted (an empty current token). File-backed forms are rejected
/// like [`interpret_dbl_array`].
pub fn interpret_dbl_array_dynamic(
    parser: &mut Parser,
    vars: &ParserVars,
    s: &str,
) -> Result<Vec<f64>, ParserError> {
    parser.set_auto_increment(false);
    parser.set_cmd_string(s);
    let parm_name = parser.next_param(vars);
    if parm_name.eq_ignore_ascii_case("file")
        || (!parm_name.is_empty()
            && (compare_text_shortest_eq(&parm_name, "dblfile")
                || compare_text_shortest_eq(&parm_name, "sngfile")))
    {
        return Err(ParserError::new(format!(
            "file-backed numeric arrays (\"{parm_name}=\") are not supported yet"
        )));
    }

    let mut out = Vec::new();
    // The current token is loaded; read it, advance, stop on the first empty
    // (exhausted) token.
    while !parser.make_string(vars).is_empty() {
        out.push(parser.make_double(vars)?);
        parser.next_param(vars);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compare_text_shortest_matches_prefixes() {
        assert!(compare_text_shortest_eq("obj", "object"));
        assert!(compare_text_shortest_eq("OBJECT", "object"));
        assert!(compare_text_shortest_eq("objectxxx", "object")); // min-length compare
        assert!(!compare_text_shortest_eq("axe", "object"));
        assert!(compare_text_shortest_eq("", "anything")); // Pascal quirk
    }

    #[test]
    fn comma_text_quotes_like_fpc() {
        // Bare tokens (no space/comma/quote) stay unquoted; the delimiter is a
        // plain comma with no separating space.
        assert_eq!(
            comma_text(&["hour".into(), "t(sec)".into(), "V1".into()]),
            "hour,t(sec),V1"
        );
        // A space (any char <= ' ') forces quoting — the monitor `Tap (pu)` /
        // `S1 (kVA)` headers.
        assert_eq!(comma_text(&["Tap (pu)".into()]), "\"Tap (pu)\"");
        assert_eq!(
            comma_text(&["S1 (kVA)".into(), "Ang1".into()]),
            "\"S1 (kVA)\",Ang1"
        );
        // An embedded comma forces quoting; an embedded quote is doubled inside
        // the wrapping quotes. (The empty-item case is out of contract — no
        // monitor header label is ever empty — so it is not asserted here.)
        assert_eq!(comma_text(&["a,b".into()]), "\"a,b\"");
        assert_eq!(comma_text(&["say \"hi\"".into()]), "\"say \"\"hi\"\"\"");
    }

    #[test]
    fn read_dbl_array_text_stops_and_shrinks_on_bad_token() {
        // Oracle-proven (dss-python 0.15.7): a `mult=(file=…)` whose row 3 is
        // `abc` raises `#705`, sets `Result := i-1` and BREAKs — the shape ends
        // up npts=2, mult=[0.1,0.2] (Pascal `Utilities.pas:515-521`).
        let (vals, err) = read_dbl_array_text("0.1\n0.2\nabc\n0.4\n", 1, false, 8);
        assert_eq!(vals, vec![0.1, 0.2]);
        assert_eq!(err, Some(3)); // 1-based failing row -> caller emits #705
    }

    #[test]
    fn read_dbl_array_text_short_file_shrinks_without_error() {
        // A clean file shorter than `max` yields fewer values (the short-file
        // `Result := i-1` via the `(F.Position+1) < F.Size` guard) and NO error.
        let (vals, err) = read_dbl_array_text("1\n2\n3\n", 1, false, 8);
        assert_eq!(vals, vec![1.0, 2.0, 3.0]);
        assert_eq!(err, None);
        // An empty/missing column stays 0.0 (Pascal `DblValue`), not an error.
        let (vals, err) = read_dbl_array_text("1,\n2,\n", 2, false, 8);
        assert_eq!(vals, vec![0.0, 0.0]);
        assert_eq!(err, None);
    }

    #[test]
    fn yes_no() {
        assert!(interpret_yes_no("yes"));
        assert!(interpret_yes_no("Y"));
        assert!(interpret_yes_no("true"));
        assert!(!interpret_yes_no("no"));
        assert!(!interpret_yes_no("False"));
        assert!(!interpret_yes_no("maybe"));
        assert!(!interpret_yes_no(""));
        assert_eq!(str_y_or_n(true), "Yes");
        assert_eq!(str_y_or_n(false), "No");
    }

    #[test]
    fn color_names_and_html() {
        // The four names confirmed live against the pinned oracle + their HTML.
        assert_eq!(
            color_to_html(interpret_color_name("Blue").unwrap()),
            "#0000FF"
        );
        assert_eq!(
            color_to_html(interpret_color_name("Green").unwrap()),
            "#008000"
        );
        assert_eq!(
            color_to_html(interpret_color_name("Red").unwrap()),
            "#FF0000"
        );
        assert_eq!(
            color_to_html(interpret_color_name("Black").unwrap()),
            "#000000"
        );
        // `$`-hex fallback (corpus `RunDSS_ckt7.dss:27` `C1=$00FF00FF`).
        assert_eq!(
            color_to_html(interpret_color_name("$00FF00FF").unwrap()),
            "#FF00FF"
        );
        // Order-sensitivity: a bare "b" matches the FIRST prefix `black`, not
        // `blue` (the Pascal if-else chain order).
        assert_eq!(interpret_color_name("b"), Some(CL_BLACK));
        // Invalid spec → None (caller emits #724 and uses clBlue).
        assert_eq!(interpret_color_name("notacolor"), None);
    }

    #[test]
    fn class_and_name_split() {
        let mut parser = Parser::new();
        let vars = ParserVars::new();
        assert_eq!(
            parse_object_class_and_name(&mut parser, &vars, "growthshape.default"),
            ("growthshape".to_string(), "default".to_string())
        );
        assert_eq!(
            parse_object_class_and_name(&mut parser, &vars, "noclass"),
            (String::new(), "noclass".to_string())
        );
        // object name passes through @var substitution
        let mut vars = ParserVars::new();
        vars.add("@name", "ld1");
        assert_eq!(
            parse_object_class_and_name(&mut parser, &vars, "load.@name"),
            ("load".to_string(), "ld1".to_string())
        );
    }

    #[test]
    fn blanks_get_quoted() {
        assert_eq!(check_for_blanks("abc"), "abc");
        assert_eq!(check_for_blanks("a b"), "\"a b\"");
        assert_eq!(check_for_blanks("(a b)"), "(a b)");
        assert_eq!(check_for_blanks("\"a b\""), "\"a b\"");
    }

    #[test]
    fn dss_arrays() {
        assert_eq!(get_dss_array_f64(2, None, 1.0), "");
        assert_eq!(
            get_dss_array_f64(2, Some(&[1.025, 1.025]), 1.0),
            "[ 1.025 1.025]"
        );
        // scale divides on the way out (Spectrum %mag stores pu, shows %)
        assert_eq!(get_dss_array_f64(2, Some(&[1.0, 0.33]), 0.01), "[ 100 33]");
        assert_eq!(get_dss_array_i32(3, Some(&[1, -2, 3])), "[ 1 -2 3]");
        assert_eq!(get_dss_array_i32(1, None), "");
    }

    #[test]
    fn float_strings() {
        assert_eq!(float_to_str(-2.0), "-2");
        assert_eq!(float_to_str(1.025), "1.025");
        assert_eq!(float_to_str_ex(f64::NAN), "----");
        assert_eq!(float_to_str_ex(0.5), "0.5");
    }

    /// FPC `FloatToStr` = `ffGeneral`/15 significant digits, NOT Rust's shortest
    /// round-trip (`{}`, up to 17 digits). Directly pins the WP8.5 fix (else only
    /// the `dump_reactor` golden gates it). Each value's shortest form needs ≥16
    /// digits; `%.15g` rounds to 15 (the `Dump`/`Save` byte-fidelity contract).
    #[test]
    fn float_to_str_is_15_sig_figs() {
        // 4.6299139469897817… — the reactor NormAmps: shortest `{}` is 16 digits.
        assert_eq!(float_to_str(4.629913946989782), "4.62991394698978");
        // 1/3: shortest is 0.3333333333333333 (16), FloatToStr → 15 sig.
        assert_eq!(float_to_str(1.0 / 3.0), "0.333333333333333");
        // 2/3 rounds the 15th digit up.
        assert_eq!(float_to_str(2.0 / 3.0), "0.666666666666667");
        // Integers + short decimals are unaffected (trailing zeros stripped).
        assert_eq!(float_to_str(60.0), "60");
        assert_eq!(float_to_str(12.47), "12.47");
        assert_eq!(float_to_str(0.0), "0");
    }
}
