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
    format!("{v}")
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

/// C `printf` `%.*g` (FPC `Format('%-.Ng', …)`): `sig` significant digits,
/// scientific notation when the decimal exponent is `< -4` or `>= sig`, with
/// trailing zeros (and a dangling decimal point) stripped. Used by the event
/// log; the gate parses these numbers out, so this only needs to be faithful,
/// not bit-exact to FPC's formatter.
pub fn fmt_g(v: f64, sig: usize) -> String {
    if v == 0.0 {
        return "0".to_string();
    }
    if !v.is_finite() {
        return float_to_str(v);
    }
    let sig = sig.max(1);
    let exp = v.abs().log10().floor() as i32;
    if exp < -4 || exp >= sig as i32 {
        // Scientific: `sig - 1` digits after the mantissa point.
        let raw = format!("{:.*e}", sig - 1, v);
        // Rust renders the exponent as `e5`/`e-4`; C uses `e+05`. The gate
        // normalizes numbers away, so split mantissa/exponent and strip the
        // mantissa's trailing zeros only.
        if let Some((mant, e)) = raw.split_once('e') {
            format!("{}e{}", strip_trailing_zeros(mant), e)
        } else {
            raw
        }
    } else {
        let decimals = (sig as i32 - 1 - exp).max(0) as usize;
        strip_trailing_zeros(&format!("{v:.decimals$}"))
    }
}

/// Drop trailing fractional zeros and any dangling `.` from a fixed-notation
/// decimal string (helper for [`fmt_g`]).
fn strip_trailing_zeros(s: &str) -> String {
    if s.contains('.') {
        let t = s.trim_end_matches('0');
        t.trim_end_matches('.').to_string()
    } else {
        s.to_string()
    }
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
}
