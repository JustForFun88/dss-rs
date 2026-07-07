//! Small string/format helpers from Pascal `Common/Utilities.pas`, ported
//! on demand (only what the engine paths implemented so far actually use).

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
/// Used by the Vsource asymmetric-matrix path; replace with the exact value
/// in the post-port cleanup pass, regenerating affected goldens.
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
/// TODO(compat): the final cut to `sig` digits is **not correctly rounded** —
/// Grisu1 first produces 17 ties-to-even digits, then `round_digits` re-rounds
/// that *decimal string* to `sig` digits **half-away-from-zero** (the FPC
/// `GRISU1_F2A_HALF_ROUNDUP` + `GRISU1_F2A_AGRESSIVE_ROUNDUP` build flags), so
/// a value just below a decimal half-boundary can round up where a correctly
/// rounded `%.Ng` rounds down (oracle-probed: `loadshape.default`'s computed
/// `FMean` = 0.82582833333333349745… → 17 digits `…33350` → prints
/// `0.825828333333334`, where correct 15-digit rounding gives `…333`). The
/// whole pipeline is pinned bit-exact against the real FPC 3.2.2 RTL by the
/// committed battery `tests/golden/fmt_battery.csv` — 13 198 values × 7
/// render forms, 0 mismatches (`crates/dss-core/tests/fmt_battery.rs`;
/// generator: `tools/fpc/fmt_battery/`) — plus the byte-exact Dump/Save
/// goldens; the clean fix (correctly rounded formatting) lands with the
/// post-acceptance compat sweep + golden regeneration.
pub fn fmt_g(v: f64, sig: usize) -> String {
    // `FloatToStrFIntl`: `If (Precision = -1) Or (Precision > maxdigits) Then
    // Precision := maxdigits` (= 15 without FPC_HAS_TYPE_EXTENDED), then
    // `Str(Double(Value) : precision + 7, Result)`.
    let precision = sig.min(15) as i32;
    let sci = grisu_str_real(precision + 7, v);
    fpc_general_post(sci)
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

/// Pascal `InterpretDblArray`, list-of-numbers path: read exactly
/// `max_values` doubles out of `s` into `out`, filling with zeros when the
/// string runs short (the parser returns 0 for an empty token). The outer
/// `Edit` parser has already stripped the surrounding `()`/`[]`, so `s` is a
/// bare delimiter-separated list; values may themselves be quoted RPN.
///
/// The `file=`/`dblfile=`/`sngfile=` spellings (file-backed arrays) are not
/// ported yet — they error out with a clear message rather than parsing the
/// keyword as a number.
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
