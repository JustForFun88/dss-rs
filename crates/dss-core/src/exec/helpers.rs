//! Free helper functions shared by the command-executive submodules
//! (split out of `exec/mod.rs`). Pascal helpers from `ExecHelper.pas`/
//! `ExecOptions.pas`.

use super::*;

/// Pascal `interpretTimeStepSize` (`ExecOptions.pas` l.315): plain number =
/// seconds; otherwise a single-char `h`/`m`/`s` suffix. On error the step size
/// is left unchanged.
pub(crate) fn interpret_time_step_size(s: &str, current_h: f64, errors: &mut Vec<String>) -> f64 {
    if let Ok(v) = s.parse::<f64>() {
        return v; // only a number was specified, so must be seconds
    }
    // Error occurred, so must have a units specifier (the last character).
    let Some(ch) = s.chars().last() else {
        errors.push(format!("Error in specification of StepSize: {s}"));
        return current_h;
    };
    let s2 = &s[..s.len() - ch.len_utf8()];
    let Ok(v) = s2.parse::<f64>() else {
        errors.push(format!("Error in specification of StepSize: {s}"));
        return current_h;
    };
    match ch {
        'h' => v * 3600.0,
        'm' => v * 60.0,
        's' => v,
        _ => {
            errors.push(format!(
                "Error in specification of StepSize: \"{s}\". Units can only be h, m, or s (single char only)"
            ));
            current_h
        }
    }
}

/// `DSS.LoadShapeClass.Find(name)`, snapshot-cloned for the circuit defaults
/// (same staleness semantics as the Load/VSource shape refs — STATUS §1c).
pub(crate) fn find_load_shape(
    classes: &[DssClass],
    name: &str,
) -> Option<load_shape::LoadShapeObj> {
    let cls = classes
        .iter()
        .find(|c| c.props.class_name().eq_ignore_ascii_case("LoadShape"))?;
    let &idx = cls.name_to_idx.get(&name.to_lowercase())?;
    cls.objects[idx]
        .as_any()
        .downcast_ref::<load_shape::LoadShapeObj>()
        .cloned()
}

/// `DSS.PriceShapeClass.Find(name)`, snapshot-cloned (`Set pricecurve=`).
pub(crate) fn find_price_shape(
    classes: &[DssClass],
    name: &str,
) -> Option<price_shape::PriceShapeObj> {
    let cls = classes
        .iter()
        .find(|c| c.props.class_name().eq_ignore_ascii_case("PriceShape"))?;
    let &idx = cls.name_to_idx.get(&name.to_lowercase())?;
    cls.objects[idx]
        .as_any()
        .downcast_ref::<price_shape::PriceShapeObj>()
        .cloned()
}

/// Pascal `Parser.DblValue` on the current token, record-and-continue.
pub(crate) fn get_dbl(
    parser: &mut Parser,
    vars: &ParserVars,
    errors: &mut Vec<String>,
) -> Option<f64> {
    match parser.make_double(vars) {
        Ok(v) => Some(v),
        Err(e) => {
            errors.push(e.message().to_string());
            None
        }
    }
}

/// Pascal `Parser.IntValue` on the current token, record-and-continue.
pub(crate) fn get_int(
    parser: &mut Parser,
    vars: &ParserVars,
    errors: &mut Vec<String>,
) -> Option<i32> {
    match parser.make_integer(vars) {
        Ok(v) => Some(v),
        Err(e) => {
            errors.push(e.message().to_string());
            None
        }
    }
}

/// `StringToOrdinal` against a registry enum, record-and-continue (the Pascal
/// exception is caught by `ProcessCommand` and logged).
pub(crate) fn enum_ord(
    enums: &EnumRegistry,
    id: EnumId,
    value: &str,
    errors: &mut Vec<String>,
) -> Option<i32> {
    match enums.get(id).string_to_ordinal(&value.to_lowercase()) {
        Ok(v) => Some(v),
        Err(e) => {
            errors.push(e.message().to_string());
            None
        }
    }
}

/// Pascal `parseIntArray` (ExecOptions.pas l.350): reparse `s` on the AuxParser
/// into an integer array. Pascal runs two passes — pass 1 counts the tokens and
/// `SetLength`s the array (zero-filling), pass 2 reads each token via `IntValue`
/// (`MakeInteger`). A token that is neither an integer nor a roundable decimal
/// makes `MakeInteger` *raise* `EParserProblem`, which the executive logs and
/// which aborts the fill — leaving the already-sized array zero-filled from the
/// bad token onward. We reproduce that exactly: the error is recorded and the
/// remaining slots stay 0 (a roundable decimal like `13.7` still rounds to 14,
/// matching the `MakeInteger` double-fallback path).
pub(crate) fn parse_int_array(
    aux_parser: &mut Parser,
    vars: &ParserVars,
    s: &str,
    errors: &mut Vec<String>,
) -> Vec<i32> {
    // Pass 1: count the tokens (StrValue never raises).
    aux_parser.set_cmd_string(s);
    let mut count = 0usize;
    loop {
        aux_parser.next_param(vars);
        if aux_parser.make_string(vars).is_empty() {
            break;
        }
        count += 1;
    }

    // Pascal `SetLength(iarray, count)` — new slots are zero-filled.
    let mut out = vec![0i32; count];

    // Pass 2: read each token as an integer, stopping at the first conversion
    // error (Pascal raises and unwinds), leaving the remaining slots at 0.
    aux_parser.set_cmd_string(s);
    for slot in &mut out {
        aux_parser.next_param(vars);
        match aux_parser.make_integer(vars) {
            Ok(v) => *slot = v,
            Err(e) => {
                errors.push(e.message().to_string());
                break;
            }
        }
    }
    out
}

/// Pascal `TExecHelper.DoAutoAddBusList` (ExecHelper.pas l.1986): parse the
/// `Set AutoBusList=` argument — either an inline bus-name list or the
/// `File=name` form (one bus name per line, resolved against the data path).
pub(crate) fn do_auto_add_bus_list(
    aux_parser: &mut Parser,
    vars: &ParserVars,
    current_dir: &Path,
    s: &str,
    out: &mut Vec<String>,
    errors: &mut Vec<String>,
) {
    out.clear();
    aux_parser.set_cmd_string(s);
    let parm_name = aux_parser.next_param(vars);
    let mut param = aux_parser.make_string(vars);

    if parm_name.eq_ignore_ascii_case("file") {
        // Load the list from a file (one bus name per line).
        let path = current_dir.join(&param);
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                for line in content.lines() {
                    aux_parser.set_cmd_string(line);
                    aux_parser.next_param(vars);
                    let p = aux_parser.make_string(vars);
                    if !p.is_empty() {
                        out.push(p);
                    }
                }
            }
            // Pascal `DoSimpleMsg('Error trying to read bus list file: %s',
            // [E.message], 268)`.
            Err(e) => errors.push(format!("Error trying to read bus list file: {e}")),
        }
    } else {
        // Parse bus names off the inline array list.
        while !param.is_empty() {
            out.push(param.clone());
            aux_parser.next_param(vars);
            param = aux_parser.make_string(vars);
        }
    }
}

/// Pascal `TExecHelper.DoKeeperBusList` (ExecHelper.pas l.2035): set the `Keep`
/// flag on buses named in the `Set KeepList=` argument — either an inline
/// bus-name list or the `File=name` form (one bus name per line, resolved
/// against the data path). **Cumulative** (unlike `AutoBusList`, it never
/// clears): to clear, use `Reset Keeplist`. Unknown bus names are silently
/// skipped (`BusList.Find` returns 0).
pub(crate) fn do_keeper_bus_list(
    aux_parser: &mut Parser,
    vars: &ParserVars,
    current_dir: &Path,
    s: &str,
    ckt: &mut Circuit,
    errors: &mut Vec<String>,
) {
    let mark = |ckt: &mut Circuit, name: &str| {
        if let Some(idx) = ckt.bus_list.find(&name.to_lowercase()) {
            ckt.buses[idx].keep = true;
        }
    };

    aux_parser.set_cmd_string(s);
    let parm_name = aux_parser.next_param(vars);
    let mut param = aux_parser.make_string(vars);

    if parm_name.eq_ignore_ascii_case("file") {
        // Load the list from a file (one bus name per line).
        let path = current_dir.join(&param);
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                for line in content.lines() {
                    aux_parser.set_cmd_string(line);
                    aux_parser.next_param(vars);
                    let p = aux_parser.make_string(vars);
                    if !p.is_empty() {
                        mark(ckt, &p);
                    }
                }
            }
            // Pascal `DoSimpleMsg('Error trying to read bus list file "%s": %s',
            // [param, E.message], 269)`.
            Err(e) => errors.push(format!(
                "Error trying to read bus list file \"{param}\": {e}"
            )),
        }
    } else {
        // Parse bus names off the inline array list.
        while !param.is_empty() {
            mark(ckt, &param);
            aux_parser.next_param(vars);
            param = aux_parser.make_string(vars);
        }
    }
}

/// Pascal `DoSetReduceStrategy` (ExecHelper.pas l.3049): parse the
/// `Set ReduceOption=` value into a [`crate::circuit::ReductionStrategy`]. The
/// first character (case-insensitive) selects the mode; an `S` is
/// disambiguated Switch-vs-Shortlines by `CompareTextShortest(S, 'SWITCH')`.
/// The stored strategy is consumed by the WP8.7 `ReduceZone` dispatch
/// (`report/reduce.rs`).
pub(crate) fn set_reduce_strategy(ckt: &mut Circuit, s: &str, errors: &mut Vec<String>) {
    use crate::circuit::ReductionStrategy as Rs;
    ckt.reduction_strategy_string = s.to_string();
    ckt.reduction_strategy = Rs::Default;
    let Some(first) = s.bytes().next() else {
        return; // No option given
    };
    ckt.reduction_strategy = match first.to_ascii_uppercase() {
        b'B' => Rs::BreakLoop,
        b'D' => Rs::Default,
        b'E' => Rs::Dangling, // Ends
        b'L' => Rs::Laterals,
        b'M' => Rs::MergeParallel,
        b'S' => {
            if crate::util::compare_text_shortest_eq(s, "SWITCH") {
                Rs::Switches
            } else {
                Rs::ShortLines
            }
        }
        _ => {
            errors.push(format!("Unknown Reduction Strategy: \"{s}\"."));
            return; // leaves rsDefault, matching Pascal
        }
    };
}

/// Pascal `AppendGlobalResult`: comma-separated accumulation.
pub(crate) fn append_result(result: &mut String, s: &str) {
    if result.is_empty() {
        result.push_str(s);
    } else {
        result.push_str(", ");
        result.push_str(s);
    }
}

pub(crate) fn yes_no(b: bool) -> &'static str {
    if b { "Yes" } else { "No" }
}

/// Pascal `IntArrayToString` (Utilities.pas): `[NULL]` when empty, else
/// `[a, b, c]`.
pub(crate) fn int_array_to_string(arr: &[i32]) -> String {
    if arr.is_empty() {
        return "[NULL]".to_string();
    }
    let mut s = String::from("[");
    for (i, v) in arr.iter().enumerate() {
        if i != 0 {
            s.push_str(", ");
        }
        s.push_str(&v.to_string());
    }
    s.push(']');
    s
}

/// Pascal `MakeLikeProperty` set path: find the source object by name in the
/// same class, clone it, and copy its state onto the target.
pub(crate) fn make_like(
    objects: &mut [Box<dyn DssObject>],
    name_to_idx: &HashMap<String, usize>,
    target: usize,
    source_name: &str,
    errors: &mut Vec<String>,
    class_name: &str,
) {
    match name_to_idx.get(&source_name.to_lowercase()) {
        Some(&si) => {
            let src = objects[si].clone_box();
            objects[target].make_like(src.as_ref());
        }
        None => {
            errors.push(format!(
                "Error in {class_name} MakeLike: \"{source_name}\" not found."
            ));
        }
    }
}

/// Pascal `ParseObjName`: split `Class.Object.Property` into `(Class.Object,
/// Property)`. With no dot, the whole string is the property name.
pub(crate) fn parse_obj_name(fullname: &str) -> (String, String) {
    match fullname.find('.') {
        None => (String::new(), fullname.to_string()),
        Some(dot1) => {
            let rest = &fullname[dot1 + 1..];
            match rest.find('.') {
                None => (fullname[..dot1].to_string(), rest.to_string()),
                Some(dot2) => (
                    fullname[..dot1 + 1 + dot2].to_string(),
                    rest[dot2 + 1..].to_string(),
                ),
            }
        }
    }
}
