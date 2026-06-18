//! The value-coercion and `SetObj*`/`GetObj*` helpers: the numeric parse
//! fallbacks plus the range/sign/scale checks Pascal applies in
//! `SetObjDouble`/`SetObjInteger`/`GetObjDouble`. Shared by [`ClassProps`]'s
//! parse and get paths.
//!
//! [`ClassProps`]: super::ClassProps

use crate::obj::base::DssObject;
use dss_parser::{ParserError, val_f64, val_i32};

use super::{PropDef, PropEngine, PropFlags};

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
    obj.set_f64(idx, value);
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
