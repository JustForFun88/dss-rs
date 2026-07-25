//! `BatchEdit` command — the r4133 form with `where` conditionals.
//!
//! Port of EPRI `Executive/ExecHelper.pas` r4133 `DoBatchEditCmd` (non-FPC
//! branch) + `DoCheckConditionals` / `DoEvalConditionals` / `DoLocalizeOp_Index`,
//! and the E1 result `GlobalResult := 'Elements edited: N'`
//! (`ExecCommands.pas` cmd 95). WP-U2.4.
//!
//! `batchedit class.pattern editstring [where cond [and|or|xor cond]...]`
//! replays `editstring` against every object of `class` whose NAME matches
//! `pattern` (regex, case-insensitive, unanchored) **and** — when a `where`
//! clause is present — satisfies the property conditionals. The Delphi tokenizer
//! is reproduced faithfully, including its quirks (probed on r4133):
//!  - the whole `where` clause is lowercased before tokenizing, so property
//!    names/values compare case-insensitively / lower-cased;
//!  - the logic operator is chosen by list order `and`, `or`, `xor` — the FIRST
//!    whose text appears **anywhere** wins, so a later `and` beats an earlier
//!    `or`, and any `xor` matches `or` first and behaves as `or`;
//!  - a missing property compares as an empty value (numeric 0), so it is
//!    silently false rather than an error;
//!  - the result string is `Elements edited: N` for **every** batchedit, with or
//!    without `where`.
//!
//! The edit itself and the model effect are unchanged from the pre-r4133 form
//! (validated live against r4133 — see `tests/corpus/modes/batchedit`).

use super::*;

/// A parsed `where` clause (Pascal globals `cond_arguments` / `cond_operators` /
/// `cond_logic_ops`). `arguments` interleaves `[var0, val0, var1, val1, ...]`;
/// `operators[i]` is the comparison for conditional `i`; `logic_ops[i]` is the
/// logic op (`0`=and, `1`=or, `2`=xor) joining conditional `i` to `i+1`.
struct BatchConditionals {
    arguments: Vec<String>,
    operators: Vec<String>,
    logic_ops: Vec<usize>,
}

/// 1-based `Pos`: byte offset+1 of the first occurrence of `needle`, or 0.
fn pos(hay: &str, needle: &str) -> usize {
    hay.find(needle).map_or(0, |i| i + 1)
}

/// Delphi `String.Substring(start)` (0-based), clamped, never panics.
fn substr_from(s: &str, start: usize) -> &str {
    s.get(start.min(s.len())..).unwrap_or("")
}

/// Delphi `String.Substring(start, len)` (0-based), clamped, never panics.
fn substr(s: &str, start: usize, len: usize) -> &str {
    let a = start.min(s.len());
    let b = (start + len).min(s.len());
    s.get(a..b.max(a)).unwrap_or("")
}

/// Port of `DoCheckConditionals`. `cond_text` is the already-lowercased text
/// **after** `where ` (leading/trailing space trimmed by the caller). Returns
/// `Ok(conds)` or `Err(msg)` for a malformed conditional (Pascal error 100240,
/// which aborts the whole batchedit with `Elements edited: 0`).
fn parse_where_conditionals(cond_text: &str) -> Result<BatchConditionals, String> {
    const LOCAL_LOGIC: [&str; 3] = ["and", "or", "xor"];
    // '>', '<', '!', '=', ' ' — the comparison-operator lead chars, space last.
    const LOCAL_OPERATORS: [&str; 5] = [">", "<", "!", "=", " "];

    let mut arguments: Vec<String> = Vec::new();
    let mut operators: Vec<String> = Vec::new();
    let mut logic_ops: Vec<usize> = Vec::new();

    let mut r_string = cond_text.trim().to_string();

    while !r_string.is_empty() {
        let mut cmd_line = r_string.clone();
        // Search for a logic operator (and/or/xor) — first in list order that
        // appears anywhere (the r4133 quirk: `and` beats an earlier `or`).
        let mut last_logic_pos = 0usize; // 1-based Pos of the logic word; 0 = none
        for (i, lg) in LOCAL_LOGIC.iter().enumerate() {
            let op_found = pos(&r_string, lg);
            if op_found > 0 {
                logic_ops.push(i);
                // cmd_line := Trim(R_String).Substring(0, op_found-2)
                cmd_line = substr(r_string.trim(), 0, op_found.saturating_sub(2)).to_string();
                last_logic_pos = op_found;
                break;
            }
        }
        // Search for the first comparison operator in cmd_line.
        let mut op_found = 0usize; // 1-based
        for op in LOCAL_OPERATORS {
            op_found = pos(&cmd_line, op);
            if op_found > 0 {
                break;
            }
        }
        let op_idx = op_found.saturating_sub(1); // 0-based index of the operator
        let mut var0 = substr(&cmd_line, 0, op_idx).trim().to_string();
        let mut elem_cond = substr(&cmd_line, op_idx, 1).to_string();
        let next_char = substr(&cmd_line, op_idx + 1, 1);
        let mut op_after = op_idx;
        if next_char == "=" {
            elem_cond.push('=');
            op_after = op_idx + 1;
        }
        let var1 = substr_from(&cmd_line, op_after + 1).trim().to_string();

        if var0.is_empty() || var1.is_empty() {
            return Err(format!(
                "Conditional wrongly declared: \"{}\". (Error 100240)",
                std::mem::take(&mut var0)
            ));
        }
        arguments.push(var0);
        arguments.push(var1);
        operators.push(elem_cond);

        if last_logic_pos == 0 {
            r_string.clear(); // reached the end of the conditionals
        } else {
            // Advance past the logic word to the next conditional.
            let s2 = substr_from(&r_string, last_logic_pos).trim().to_string();
            let sp = pos(&s2, " "); // Pos(' ', ...) 1-based; 0 = none
            r_string = substr_from(&s2, sp).trim().to_string();
        }
    }

    Ok(BatchConditionals {
        arguments,
        operators,
        logic_ops,
    })
}

impl Dss {
    /// Pascal `DoBatchEditCmd` (`ExecHelper.pas` r4133):
    /// `BatchEdit class.pattern editstring [where …]` — replay the edit string
    /// against every object of the class whose NAME matches the regex pattern
    /// **case-insensitively and unanchored** (`TPerlRegEx` `preCaseLess`) and, if
    /// a `where` clause is present, whose properties satisfy the conditionals.
    /// Sets `GlobalResult := 'Elements edited: N'` (E1).
    pub(super) fn do_batch_edit_cmd(&mut self) {
        let (obj_class, pattern) = self.get_obj_class_and_name();
        if obj_class.eq_ignore_ascii_case("circuit") {
            self.last_result = "Elements edited: 0".to_string();
            return; // Do nothing
        }
        let Some(&ci) = self.class_by_name.get(&obj_class.to_ascii_lowercase()) else {
            // Pascal error 267 (ExecHelper.pas:313; the `%s` is
            // `CRLF + Parser.CmdString`; LF here, same rendering as error 240
            // in `get_obj_class_and_name`).
            self.errors.push(crate::diag::DssDiagnostic::msg(
                format!(
                    "BatchEdit Command: Object Type \"{obj_class}\" not found. \n{}",
                    self.parser.cmd_string()
                ),
                Some(267),
            ));
            self.last_result = "Elements edited: 0".to_string();
            return;
        };
        self.active_class = Some(ci); // DSS.LastClassReferenced / ActiveDSSClass
        // `Params := DSS.Parser.Position` — the edit string starts here.
        let params_pos = self.parser.position();

        // r4133 `DoCheckConditionals`: parse an optional `where` clause from the
        // remainder. The clause is lowercased for tokenizing; the edit string
        // (everything before `where`) keeps its original case.
        let remainder = self.parser.remainder().to_string();
        let lower = remainder.to_ascii_lowercase();
        let where_pos = lower.find("where"); // naive Pos, first occurrence
        let conditionals = match where_pos {
            None => None,
            Some(wp) => {
                // Trim(R_String.Substring(cond_pos+5)) — skip `where` (5) + 1 char.
                let start = (wp + 6).min(lower.len());
                match parse_where_conditionals(lower[start..].trim()) {
                    // An empty `where` clause parses zero conditionals — Pascal
                    // `DoCheckConditionals` returns 0 (not 1), so it is treated as
                    // no filter (edit every match). The `where` text is still
                    // stripped from the edit string below (keyed on `where_pos`).
                    Ok(c) if c.operators.is_empty() => None,
                    Ok(c) => Some(c),
                    Err(msg) => {
                        self.errors.push(msg);
                        self.last_result = "Elements edited: 0".to_string();
                        return;
                    }
                }
            }
        };

        let re = match regex::RegexBuilder::new(&pattern)
            .case_insensitive(true) // TPerlRegEx `preCaseLess`
            .build()
        {
            Ok(re) => re,
            Err(e) => {
                // A bad pattern surfaces as an engine error; the exact upstream
                // text is FPC/Perl-internal, so record the regex error.
                self.errors.push(format!("BatchEdit Command: {e}"));
                self.last_result = "Elements edited: 0".to_string();
                return;
            }
        };

        // With a `where` clause, r4133 rebuilds the command string without the
        // conditionals so the ordinary Edit sees only the params. We instead load
        // the edit string (before `where`) directly and rewind to its start for
        // each match — observably identical (the edit params are the same).
        let edit_pos = if let Some(wp) = where_pos {
            let edit_str = remainder[..wp].trim();
            self.parser.set_cmd_string(edit_str);
            self.parser.position()
        } else {
            params_pos
        };

        // `First`/`Next`: walk the class list in creation order; every object is
        // visited (active), the edit runs only on a regex match AND (if present) a
        // satisfied conditional.
        let mut count = 0usize;
        for oi in 0..self.classes[ci].arena.len() {
            self.classes[ci].active = Some(oi);
            let apply = match &conditionals {
                None => true,
                Some(conds) => match self.eval_batch_conditionals(ci, oi, conds) {
                    Ok(b) => b,
                    Err(msg) => {
                        // Pascal error 100241/100243: stop the walk.
                        self.errors.push(msg);
                        break;
                    }
                },
            };
            if !apply {
                continue;
            }
            let name = self.classes[ci].arena[oi].data().name().to_string();
            if re.is_match(&name) {
                self.parser.set_position(edit_pos);
                self.edit_active();
                count += 1;
            }
        }

        // E1 (ExecCommands.pas cmd 95): GlobalResult := 'Elements edited: N'.
        self.last_result = format!("Elements edited: {count}");
    }

    /// Pascal `DoEvalConditionals`: evaluate the parsed conditionals against the
    /// object `oi` of class `ci`. `>`/`<`/`>=`/`<=` compare property vs value as
    /// doubles (`AuxParser.DblValue`), `=`/`!=` as strings; a missing property
    /// reads as `""` (numeric 0). Conditionals are combined left-to-right by the
    /// logic ops. `Err` is a malformed operator (Pascal 100241) that aborts.
    fn eval_batch_conditionals(
        &self,
        ci: usize,
        oi: usize,
        conds: &BatchConditionals,
    ) -> Result<bool, String> {
        let cls = &self.classes[ci];
        let obj = cls.arena.obj(oi);
        let mut results: Vec<bool> = Vec::with_capacity(conds.operators.len());
        for i in 0..conds.operators.len() {
            let name = &conds.arguments[i * 2];
            let user_val = &conds.arguments[i * 2 + 1];
            // `ActiveDSSClass.PropertyIndex` — exact (case-insensitive) name
            // match, NOT abbreviated (mirrors Delphi `TDSSClass.PropertyIndex`).
            let prop_val = (1..=cls.props.num_properties())
                .find(|&idx| cls.props.property_name(idx).eq_ignore_ascii_case(name))
                .map(|idx| cls.props.get_value(obj, idx, &self.enums))
                .unwrap_or_default(); // GetPropertyValue(0) -> ""
            let r = match conds.operators[i].as_str() {
                ">" => fpc_dbl(&prop_val, &self.vars) > fpc_dbl(user_val, &self.vars),
                "<" => fpc_dbl(&prop_val, &self.vars) < fpc_dbl(user_val, &self.vars),
                ">=" => fpc_dbl(&prop_val, &self.vars) >= fpc_dbl(user_val, &self.vars),
                "<=" => fpc_dbl(&prop_val, &self.vars) <= fpc_dbl(user_val, &self.vars),
                "=" => *user_val == prop_val,
                "!=" => *user_val != prop_val,
                other => {
                    return Err(format!(
                        "Operator not identified/supported: \"{other}\". (Error 100241)"
                    ));
                }
            };
            results.push(r);
        }
        // Combine left-to-right (Pascal `logic_probe`).
        let Some(&first) = results.first() else {
            return Ok(false); // no conditionals (defensive; caller filters empty)
        };
        let mut probe = first;
        for (k, &lop) in conds.logic_ops.iter().enumerate() {
            let Some(&local) = results.get(k + 1) else {
                break;
            };
            probe = match lop {
                0 => probe && local, // and
                1 => probe || local, // or
                2 => probe ^ local,  // xor
                _ => {
                    return Err(
                        "The logic test is not identified/supported. (Error 100242)".to_string()
                    );
                }
            };
        }
        Ok(probe)
    }
}

/// FPC `AuxParser.DblValue` on a value string: parse the first token as a
/// double, `0.0` on any conversion failure (matching the empty/missing-property
/// case that evaluates conditionals to false).
fn fpc_dbl(s: &str, vars: &ParserVars) -> f64 {
    let mut p = Parser::new();
    p.set_cmd_string(s);
    p.next_param(vars);
    p.make_double(vars).unwrap_or(0.0)
}
