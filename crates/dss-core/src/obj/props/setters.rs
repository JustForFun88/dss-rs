//! The value-coercion and `SetObj*`/`GetObj*` helpers: the numeric parse
//! fallbacks plus the range/sign/scale checks Pascal applies in
//! `SetObjDouble`/`SetObjInteger`/`GetObjDouble`. Shared by [`ClassProps`]'s
//! parse and get paths.
//!
//! [`ClassProps`]: super::ClassProps

use crate::obj::base::DssObject;
use dss_parser::{ParserError, val_f64, val_i32};

use super::{PropDef, PropEngine, PropFlags, PropType};

/// Pascal `ParseObjPropertyValue.GetDouble`: try FPC `Val` first, falling back
/// to the parser (so RPN expressions like `"2 3 *"` work) by wrapping in `()`.
pub(super) fn get_double(eng: &mut PropEngine, value: &str) -> Result<f64, ParserError> {
    if let Some(v) = val_f64(value) {
        return Ok(v);
    }
    eng.parser.set_auto_increment(false);
    eng.parser.set_cmd_string(&format!("({value})"));
    eng.parser.next_param(eng.vars);
    eng.parser.make_double(eng.vars)
}

/// Pascal `ParseObjPropertyValue.GetInteger`.
pub(super) fn get_integer(eng: &mut PropEngine, value: &str) -> Result<i32, ParserError> {
    if let Some(v) = val_i32(value) {
        return Ok(v);
    }
    eng.parser.set_auto_increment(false);
    eng.parser.set_cmd_string(&format!("({value})"));
    eng.parser.next_param(eng.vars);
    eng.parser.make_integer(eng.vars)
}

/// Pascal `IntervalUnits` integer parse (`DSSObjectHelper.pas` l.325): try the
/// whole string as an integer (FPC `Val`); on failure, strip the trailing char as
/// a time-unit suffix — `h` (×3600), `m` (×60), `s` (×1) — and parse the prefix.
/// A bare number is seconds. Returns `None` on a bad number (error 2020034) or a
/// bad unit char (2020035); the caller logs the message and leaves the field
/// unchanged (Pascal `Exit`). The suffix is lowercase only, matching the Pascal
/// `case` (no implicit case-folding).
pub(super) fn parse_interval_units_i32(value: &str) -> Option<i32> {
    if let Some(v) = val_i32(value) {
        return Some(v);
    }
    let (prefix, last) = split_units_suffix(value)?;
    let base = val_i32(prefix)?;
    match last {
        's' => Some(base),
        'm' => Some(base * 60),
        'h' => Some(base * 3600),
        _ => None,
    }
}

/// Pascal `IntervalUnits` double parse (`DSSObjectHelper.pas` l.273) — the Double
/// twin of [`parse_interval_units_i32`].
pub(super) fn parse_interval_units_f64(value: &str) -> Option<f64> {
    if let Some(v) = val_f64(value) {
        return Some(v);
    }
    let (prefix, last) = split_units_suffix(value)?;
    let base = val_f64(prefix)?;
    match last {
        's' => Some(base),
        'm' => Some(base * 60.0),
        'h' => Some(base * 3600.0),
        _ => None,
    }
}

/// Split a value into (prefix, last char) for the units-suffix parse (Pascal
/// `Copy(Value, 1, Length-1)` + `Value[High(Value)]`); `None` if empty.
fn split_units_suffix(value: &str) -> Option<(&str, char)> {
    let last = value.chars().next_back()?;
    Some((&value[..value.len() - last.len_utf8()], last))
}

/// The Pascal `IntervalUnits` error message (codes 2020034/2020035), shared by
/// the Integer and Double parse arms.
pub(super) fn interval_units_error(full: &str, name: &str, value: &str) -> String {
    format!(
        "{full}.{name}: Error in specification, invalid value: \"{value}\". \
         Units can only be h, m, or s (single char only). If omitted, \"s\" is assumed."
    )
}

/// Pascal `SetObjDouble`: apply scale and the range/sign checks, the zero trap,
/// and `InverseValue`, then write. A failed check records a message and leaves
/// the field untouched (the field keeps its previous value).
pub(super) fn set_obj_double(
    pd: &PropDef,
    obj: &mut dyn DssObject,
    idx: usize,
    mut value: f64,
    scale: f64,
    eng: &mut PropEngine,
    full: &str,
) {
    let f = pd.flags;
    let ignore = f.contains(PropFlags::IGNORE_INVALID);
    if f.contains(PropFlags::GREATER_THAN_ONE) && value <= 1.0 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) must be greater than one.",
                pd.name
            ));
        }
        return;
    }
    if f.contains(PropFlags::NON_ZERO) && value == 0.0 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) cannot be zero.",
                pd.name
            ));
        }
        return;
    }
    if f.contains(PropFlags::NON_NEGATIVE) && value < 0.0 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) cannot be negative.",
                pd.name
            ));
        }
        return;
    }
    if f.contains(PropFlags::NON_POSITIVE) && value > 0.0 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) cannot be positive.",
                pd.name
            ));
        }
        return;
    }

    value *= scale;
    if value == 0.0 && pd.trap_zero != 0.0 {
        value = pd.trap_zero;
    }
    if value != 0.0 && f.contains(PropFlags::INVERSE_VALUE) {
        value = 1.0 / value;
    }
    // Pascal `SetObjDouble`'s trailing `case PropertyType` writes ONLY the
    // scalar double types (`DoubleProperty` / `DoubleOnArrayProperty` /
    // `DoubleOnStructArrayProperty`); every array/matrix type falls through with
    // no write. The string edit path only reaches here for `PropType::Double`,
    // but the MakePosSequence typed setter (`ClassProps::set_prop_f64`) may aim a
    // `SetDouble` at a `DoubleArrayProperty` — e.g. `TCapacitorObj.MakePosSequence`
    // does `SetDouble(ord(TProp.Cuf), Cs - Cm)` on the `Cuf` array. Upstream that
    // is a silent no-op on the value (oracle-verified: `cuf` is unchanged across
    // `makeposseq`); the seq-mark + side effects still run (in the caller, since
    // `ErrorNumber` stays 0). Mirror the fall-through: skip the write for the
    // non-scalar types instead of panicking in the element's scalar `set_f64`.
    if pd.ptype == PropType::Double {
        obj.set_f64(idx, value);
    }
}

/// Pascal `GetObjDouble`: divide by scale on the way out (and invert under
/// `InverseValue`), the mirror of [`set_obj_double`].
pub(super) fn get_obj_double(pd: &PropDef, obj: &dyn DssObject, idx: usize, scale: f64) -> f64 {
    let raw = obj.get_f64(idx);
    if pd.flags.contains(PropFlags::INVERSE_VALUE) {
        1.0 / (raw / scale)
    } else {
        raw / scale
    }
}

/// Pascal `SetObjInteger`: the range/sign checks plus `ValueOffset`, returning
/// the previous value (captured before the write) for side effects.
pub(super) fn set_obj_integer(
    pd: &PropDef,
    obj: &mut dyn DssObject,
    idx: usize,
    mut value: i32,
    eng: &mut PropEngine,
    full: &str,
) -> i32 {
    let f = pd.flags;
    let ignore = f.contains(PropFlags::IGNORE_INVALID);
    let prev = obj.get_i32(idx);
    if f.contains(PropFlags::GREATER_THAN_ONE) && value <= 1 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) must be greater than one.",
                pd.name
            ));
        }
        return prev;
    }
    if f.contains(PropFlags::NON_ZERO) && value == 0 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) cannot be zero.",
                pd.name
            ));
        }
        return prev;
    }
    if f.contains(PropFlags::NON_NEGATIVE) && value < 0 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) cannot be negative.",
                pd.name
            ));
        }
        return prev;
    }
    if f.contains(PropFlags::NON_POSITIVE) && value > 0 {
        if !ignore {
            eng.errors.push(format!(
                "{full}.{}: Value ({value}) cannot be positive.",
                pd.name
            ));
        }
        return prev;
    }
    if f.contains(PropFlags::VALUE_OFFSET) {
        value += pd.value_offset.round_ties_even() as i32;
    }
    obj.set_i32(idx, value);
    prev
}

#[cfg(test)]
mod tests {
    use super::{parse_interval_units_f64, parse_interval_units_i32};

    #[test]
    fn interval_units_i32_suffixes() {
        // Bare = seconds; s = x1, m = x60, h = x3600 (Pascal IntervalUnits).
        assert_eq!(parse_interval_units_i32("2"), Some(2));
        assert_eq!(parse_interval_units_i32("2s"), Some(2));
        assert_eq!(parse_interval_units_i32("5m"), Some(300));
        assert_eq!(parse_interval_units_i32("1h"), Some(3600));
        assert_eq!(parse_interval_units_i32("0s"), Some(0));
    }

    #[test]
    fn interval_units_i32_rejects_bad_input() {
        // Bad unit char, non-integer prefix, uppercase (Pascal `case` is lowercase
        // only), and empty all yield None -> the caller logs the error and leaves
        // the field unchanged.
        assert_eq!(parse_interval_units_i32("2x"), None);
        assert_eq!(parse_interval_units_i32("2.5"), None);
        assert_eq!(parse_interval_units_i32("xs"), None);
        assert_eq!(parse_interval_units_i32("2S"), None); // uppercase not folded
        assert_eq!(parse_interval_units_i32(""), None);
    }

    #[test]
    fn interval_units_f64_suffixes() {
        assert_eq!(parse_interval_units_f64("2.5"), Some(2.5));
        assert_eq!(parse_interval_units_f64("2.5s"), Some(2.5));
        assert_eq!(parse_interval_units_f64("0.5m"), Some(30.0));
        assert_eq!(parse_interval_units_f64("0.25h"), Some(900.0));
        assert_eq!(parse_interval_units_f64("2H"), None); // uppercase not folded
        assert_eq!(parse_interval_units_f64("2x"), None);
    }
}
