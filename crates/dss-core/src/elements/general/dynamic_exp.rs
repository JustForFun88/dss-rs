//! `DynamicExp` (`TDynamicExpObj`): a user-defined differential-equation object
//! evaluated during a dynamics solve. Port of Pascal `General/DynamicExp.pas`.
//!
//! A `DynamicExp` declares a set of state variables (`VarNames`) and an
//! `Expression` — a sequence of `<var> dt = <RPN expression>;` clauses written
//! in Reverse-Polish notation. Setting `Expression` compiles it (once) into a
//! flat `cmds` automation array (`InterpretDiffEq`) that a fast stack evaluator
//! (`SolveEq`) walks each dynamics step over a per-element memory space.
//!
//! Scope (WP7.3 step 0): this lands the object + its expression interpreter and
//! evaluator only. The dynamic-expression *integration* (a `DynEqPCE` driving
//! `SolveEq` over `DynaVars.h`) is WP7.7; porting the object here lets PVSystem/
//! Storage/Generator resolve `DynamicEq` as a real object reference rather than a
//! `NOT_PORTED` string.
//!
//! Pascal `TDynamicExpProp`: `NVariables=1`, `VarNames=2`, `vr=3` (`var`),
//! `VarIdx=4`, `Expression=5`, `Domain=6`; the base class appends `Like=7`.

#[cfg(test)]
mod tests;

use dss_parser::{Parser, ParserVars, RPNCalculator, val_f64};

use crate::obj::base::{DssObjData, DssObject};
use crate::obj::props::{PropDef, PropFlags, define_properties};

define_properties! {
    class "DynamicExp", abbrev true, enums enums;
    1 NVARIABLES => PropDef::integer("NVariables").flags(PropFlags::SUPPRESS_JSON);
    2 VARNAMES   => PropDef::string_list("VarNames").flags(PropFlags::TRANSFORM_LOWERCASE);
    3 VR         => PropDef::string("Var")
        .flags(PropFlags::TRANSFORM_LOWERCASE | PropFlags::SUPPRESS_JSON);
    4 VARIDX     => PropDef::integer("VarIdx").flags(PropFlags::SUPPRESS_JSON);
    5 EXPRESSION => PropDef::string("Expression");
    6 DOMAIN     => PropDef::mapped_string_enum("Domain", enums.dynamic_exp_domain);
}

use prop::{DOMAIN, EXPRESSION, NVARIABLES, VARIDX, VARNAMES, VR};

/// Pascal `DYN_SLOT_LENGTH`: each state variable owns a 2-slot memory cell —
/// `[value, derivative]` (one `z-1` slot for now).
pub const DYN_SLOT_LENGTH: usize = 2;

/// Pascal `opCodes` (index 0..28): the operators `Get_Closer_Op`/`InterpretDiffEq`
/// recognise, in priority order. The negative of the index is the operation code
/// stored in `cmds` (e.g. `+` at index 2 → `-2 = Add`).
const OP_CODES: [&str; 29] = [
    "dt", "=", "+", "-", "*", "/", "(", ")", ";", "[", "]", "sqr", "sqrt", "inv", "ln", "exp",
    "log10", "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "rollup", "rolldn", "swap",
    "pi", "^",
];

/// Pascal `Get_Var_Idx` returns this sentinel for a numeric constant (vs a
/// negative for "not found", or a non-negative state-variable index).
const CONST_CODE: i32 = 50001;
/// Pascal `cmds` offset marking a constant: `50000 + index-into-VarConsts`.
const CONST_BASE: i32 = 50000;
/// Pascal `cmds` marker for the start of a new equation (after the output var).
const EQ_MARK: i32 = -50;

/// `TDynamicExpObj`.
#[derive(Debug, Clone)]
pub struct DynamicExpObj {
    data: DssObjData,
    /// Index of the active variable (`var=`), -1 when none/invalid.
    var_idx: i32,
    /// State-variable names (lowercased on set).
    var_names: Vec<String>,
    /// Numeric constants pulled out of the expression (referenced from `cmds`).
    var_consts: Vec<f64>,
    /// Compiled command sequence (`InterpretDiffEq` output).
    cmds: Vec<i32>,
    /// Name of the active variable (`var=`, lowercased).
    active_var: String,
    /// The differential equation in RPN, as written (kept verbatim; cleared on
    /// a compile error).
    expression: String,
    /// Number of state variables (`NVariables`; "not really used", per Pascal).
    n_variables: i32,
    /// `Domain` ordinal (0 = Time, 1 = dq).
    domain: i32,
}

impl DynamicExpObj {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            data: DssObjData::new(name.into().to_lowercase(), prop::NUM_PROPS),
            var_idx: -1,
            var_names: Vec::new(),
            var_consts: Vec::new(),
            cmds: Vec::new(),
            active_var: String::new(),
            expression: String::new(),
            n_variables: 0,
            domain: 0, // TDynDomain.Time
        }
    }

    /// Pascal `NVariables` property — the declared variable count
    /// (`nvariables=`). `TDynEqPCE` sizes its `DynamicEqVals` memory and its
    /// `NumVariables` by this (`* DYN_SLOT_LENGTH`), not by `var_names.len()`.
    pub fn n_variables(&self) -> i32 {
        self.n_variables
    }

    /// Pascal `Get_Var_Idx`: the index of `var_name` in the state-variable list,
    /// or [`CONST_CODE`] if it parses as a numeric constant, or -1 otherwise.
    pub fn get_var_idx(&self, var_name: &str) -> i32 {
        let lower = var_name.to_lowercase();
        if let Some(i) = self.var_names.iter().position(|n| *n == lower) {
            return i as i32;
        }
        // Not a state variable — maybe a constant (Pascal `strtofloat` in a
        // try/except: success ⇒ 50001, failure ⇒ -1).
        if val_f64(&lower).is_some() {
            CONST_CODE
        } else {
            -1
        }
    }

    /// Pascal `Get_Closer_Op`: scan `expr` for the operator with the smallest
    /// (leftmost) 1-based position. Returns `(position, op_string, op_num)`;
    /// position is `10000` when no operator is present. A `-` immediately
    /// followed by a non-space is treated as a negative number, not an operator.
    fn get_closer_op(expr: &str) -> (usize, &'static str, i32) {
        let bytes = expr.as_bytes();
        let mut result = 10000usize;
        let mut op_code = "";
        let mut op_num = -1;
        for (idx, code) in OP_CODES.iter().enumerate() {
            // Pascal `Pos`: 1-based index of the first occurrence, 0 if absent.
            let op_pos = match expr.find(code) {
                Some(p) => p + 1,
                None => 0,
            };
            if op_pos >= result || op_pos == 0 {
                continue;
            }
            // Verify a `-` is the operator, not the sign of a negative number:
            // `expr[op_pos + 1]` (1-based) is the char right after the `-`.
            if *code == "-" {
                let after = bytes.get(op_pos); // 0-based op_pos == 1-based op_pos+1
                if after != Some(&b' ') {
                    continue;
                }
            }
            result = op_pos;
            op_code = code;
            op_num = idx as i32;
        }
        (result, op_code, op_num)
    }

    /// Pascal `Get_Out_Idx`: the index of `var_name` if it is a state variable
    /// *and* an output (its slot is immediately followed by an [`EQ_MARK`] in
    /// `cmds`), or -1.
    pub fn get_out_idx(&self, var_name: &str) -> i32 {
        let lower = var_name.to_lowercase();
        for (idx, name) in self.var_names.iter().enumerate() {
            if *name != lower {
                continue;
            }
            for cmd_idx in 0..self.cmds.len() {
                if self.cmds[cmd_idx] == idx as i32
                    && cmd_idx + 1 < self.cmds.len()
                    && self.cmds[cmd_idx + 1] == EQ_MARK
                {
                    return idx as i32;
                }
            }
        }
        -1
    }

    /// Pascal `Get_VarName`: the printable name of memory slot `idx` — the base
    /// variable name prefixed by `d`/`dN` for the derivative columns.
    pub fn get_var_name(&self, idx: usize) -> String {
        let len = DYN_SLOT_LENGTH;
        let row = idx / len;
        let col = idx - (row * len);
        let mut diffstr = String::new();
        if col > 0 {
            diffstr.push('d');
            if col > 1 {
                diffstr.push_str(&col.to_string());
            }
        }
        format!("{diffstr}{}", self.var_names[row])
    }

    /// Pascal `Get_DynamicEqVal`: read memory slot `idx` (row = variable,
    /// column = value/derivative) out of `mem_space`.
    pub fn get_dynamic_eq_val(idx: usize, mem_space: &[[f64; DYN_SLOT_LENGTH]]) -> f64 {
        let len = DYN_SLOT_LENGTH;
        let row = idx / len;
        let col = idx - (row * len);
        mem_space[row][col]
    }

    /// Pascal `IsInitVal`: codes 7/8/9 (`p0`/`q0`/`edp` in `Check_If_CalcValue`)
    /// are initialization values.
    pub fn is_init_val(code: i32) -> bool {
        // Pascal `case code of 7, 8, 9` (p0/q0/edp): a contiguous range.
        matches!(code, 7..=9)
    }

    /// Pascal `Check_If_CalcValue`: whether `value_str` names a value the host
    /// element computes from its model (returns the 0-based code, else -1).
    pub fn check_if_calc_value(value_str: &str) -> i32 {
        const VAL_NAMES: [&str; 12] = [
            "p", "q", "vmag", "vang", "imag", "iang", "s", "p0", "q0", "edp", "kvdc", "mod",
        ];
        let lower = value_str.to_lowercase();
        VAL_NAMES
            .iter()
            .position(|n| *n == lower)
            .map_or(-1, |i| i as i32)
    }

    /// The number of state variables this expression declares (`var_names`
    /// length) — what a host PCE sizes its memory space by.
    pub fn num_state_vars(&self) -> usize {
        self.var_names.len()
    }

    /// Pascal `SolveEq`: evaluate every compiled equation over `mem_space`,
    /// writing each output variable's derivative into column 1 of its row.
    /// `mem_space` must have at least [`Self::num_state_vars`] rows.
    pub fn solve_eq(&self, mem_space: &mut [[f64; DYN_SLOT_LENGTH]]) {
        let mut rpn = RPNCalculator::new();
        let mut out_idx: i32 = -1;
        for idx in 0..self.cmds.len() {
            // The start of a new equation is `[outVar, EQ_MARK, ...]`: detect it
            // by the slot immediately preceding an EQ_MARK, or the EQ_MARK itself.
            // (Pascal reads `Cmds[idx + 1]` unguarded; past the end there is no
            // EQ_MARK, so the guarded `.get` reproduces the intent without the UB.)
            let next_is_mark = self.cmds.get(idx + 1) == Some(&EQ_MARK);
            if next_is_mark || self.cmds[idx] == EQ_MARK {
                if self.cmds[idx] != EQ_MARK {
                    // It's the output-variable index of a new equation.
                    if out_idx >= 0 {
                        // Upload the previous equation's result.
                        mem_space[out_idx as usize][1] = rpn.get_x();
                    }
                    out_idx = self.cmds[idx];
                }
                continue;
            }
            match self.cmds[idx] {
                -2 => rpn.add(),
                -3 => rpn.subtract(),
                -4 => rpn.multiply(),
                -5 => rpn.divide(),
                -11 => rpn.square(),
                -12 => rpn.sqrt(),
                -13 => rpn.inv(),
                -14 => rpn.nat_log(),
                -15 => rpn.etothex(),
                -16 => rpn.ten_log(),
                -17 => rpn.sin_deg(),
                -18 => rpn.cos_deg(),
                -19 => rpn.tan_deg(),
                -20 => rpn.asin_deg(),
                -21 => rpn.acos_deg(),
                -22 => rpn.atan_deg(),
                -23 => rpn.atan2_deg(),
                -24 => rpn.roll_up(),
                -25 => rpn.roll_down(),
                -26 => rpn.swap_xy(),
                -27 => rpn.enter_pi(),
                -28 => rpn.y_to_the_x_power(),
                code => {
                    if code >= CONST_BASE {
                        rpn.set_x(self.var_consts[(code - CONST_BASE) as usize]);
                    } else {
                        rpn.set_x(mem_space[code as usize][0]);
                    }
                }
            }
        }
        if out_idx >= 0 {
            mem_space[out_idx as usize][1] = rpn.get_x();
        }
    }

    /// Pascal `InterpretDiffEq`: compile `self.expression` into the `cmds`
    /// automation array (and harvest numeric constants into `var_consts`). On a
    /// reference error the expression is cleared and the failure is logged,
    /// matching the original (errors 50005/50003, or 50006 for a constant where
    /// a `dt` output variable was required).
    fn interpret_diff_eq(&mut self) {
        let mut error_src = String::new();
        let mut got_error = false;
        self.cmds.clear();
        // Pascal does NOT clear VarConsts here; new constants append and the
        // freshly-built cmds reference the new (correct) indices.
        let full_expr = format!("[{}]", self.expression.to_lowercase());
        let mut expr = full_expr;

        while !expr.is_empty() {
            let (mut op_idx, op, op_code) = Self::get_closer_op(&expr);
            if op_idx == 10000 {
                expr = String::new(); // done
                continue;
            }
            // Pascal `Expr.Substring(0, OpIdx - 1)`: the text before the operator.
            let sub_xp = substring(&expr, 0, op_idx - 1);
            if op.len() > 1 {
                op_idx += op.len();
            }
            expr = substring(&expr, op_idx, expr.len());
            let vars = interpret_string_list(&sub_xp);

            match op_code {
                0 => {
                    // `dt`: the preceding token is the equation's output variable.
                    // Pascal accesses `vars[0]` directly; on an empty preceding
                    // sub-expression that raises `EStringListError` ("List index
                    // (0) out of bounds"), which the command processor catches —
                    // the error is logged, the expression is left untouched (the
                    // unwind skips the clear-on-error path), and processing
                    // continues. Reproduce it as a recoverable error + early
                    // return rather than a panic (probed against the oracle).
                    let Some(out_var) = vars.first() else {
                        self.data.push_error("List index (0) out of bounds");
                        return;
                    };
                    let idx = self.get_var_idx(out_var);
                    if idx == CONST_CODE {
                        self.data.push_error(
                            "DynamicExp: the expression preceeding the \"dt\" operand has to be a \
                             state variable.",
                        );
                        // Pascal `Exit`s the whole procedure here: the expression
                        // is NOT cleared and `gotError` stays false.
                        return;
                    } else if idx < 0 {
                        error_src = vars[0].clone();
                    } else {
                        self.cmds.push(idx);
                        self.cmds.push(EQ_MARK);
                    }
                }
                1 | 6 | 7 | 8 | 9 => {
                    // `=` / `(` / `)` / `;` / `[`: notation only, nothing emitted.
                }
                _ => {
                    // A basic operation, a function, or `]` (end): push the
                    // operands, then (unless `]`) the operator code.
                    for (i, token) in vars.iter().enumerate() {
                        let idx = self.get_var_idx(token);
                        if idx == CONST_CODE {
                            self.var_consts
                                .push(val_f64(&token.to_lowercase()).unwrap_or(0.0));
                            self.cmds
                                .push(CONST_BASE + (self.var_consts.len() as i32 - 1));
                        } else if idx < 0 {
                            // Pascal reports vars[0], not the offending vars[i].
                            let _ = i;
                            error_src = format!("\"{}\"", vars[0]);
                        } else {
                            self.cmds.push(idx);
                        }
                    }
                    if op_code != 10 {
                        // not `]`
                        self.cmds.push(-op_code);
                    }
                }
            }

            if !error_src.is_empty() {
                self.data
                    .push_error(format!("DynamicExp: Variable \"{error_src}\" not Found."));
                expr = String::new();
                got_error = true;
            }
        }

        if got_error {
            self.expression = String::new();
            self.data
                .push_error("There are errors in the differential equation.");
        }
    }
}

impl DssObject for DynamicExpObj {
    fn data(&self) -> &DssObjData {
        &self.data
    }
    fn data_mut(&mut self) -> &mut DssObjData {
        &mut self.data
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_i32(&self, idx: usize) -> i32 {
        match idx {
            NVARIABLES => self.n_variables,
            VARIDX => self.var_idx,
            DOMAIN => self.domain,
            _ => unreachable!("DynamicExp has no integer property {idx}"),
        }
    }
    fn set_i32(&mut self, idx: usize, value: i32) {
        match idx {
            NVARIABLES => self.n_variables = value,
            // Pascal `VarIdx` is `SilentReadOnly`: a direct write is ignored
            // (the field is driven only by the `var=` side effect).
            VARIDX => {}
            DOMAIN => self.domain = value,
            _ => unreachable!("DynamicExp has no integer property {idx}"),
        }
    }

    fn get_string(&self, idx: usize) -> String {
        match idx {
            VR => self.active_var.clone(),
            EXPRESSION => self.expression.clone(),
            _ => unreachable!("DynamicExp has no string property {idx}"),
        }
    }
    fn set_string(&mut self, idx: usize, value: String) {
        match idx {
            VR => self.active_var = value,
            EXPRESSION => self.expression = value,
            _ => unreachable!("DynamicExp has no string property {idx}"),
        }
    }

    fn get_string_list(&self, idx: usize) -> Vec<String> {
        debug_assert_eq!(idx, VARNAMES);
        self.var_names.clone()
    }
    fn set_string_list(&mut self, idx: usize, value: Vec<String>) {
        debug_assert_eq!(idx, VARNAMES);
        self.var_names = value;
    }

    fn side_effects(&mut self, idx: usize, _prev_int: i32) {
        match idx {
            // `NVariables`: "Not really used, ignore" (Pascal).
            EXPRESSION => self.interpret_diff_eq(),
            VR => {
                // Pascal: `VarIdx := VarNames.IndexOf(ActiveVar)`; a miss logs
                // 50001 and clears the active variable.
                self.var_idx = self
                    .var_names
                    .iter()
                    .position(|n| *n == self.active_var)
                    .map_or(-1, |i| i as i32);
                if self.var_idx < 0 {
                    self.data.push_error(format!(
                        "DynamicExp variable \"{}\" not found.",
                        self.active_var
                    ));
                    self.active_var = String::new();
                }
            }
            _ => {}
        }
    }

    fn make_like(&mut self, _other: &dyn DssObject) {
        // Pascal `TDynamicExpObj.MakeLike` only logs an error — it copies
        // nothing (not even the base PrpSequence), so a `like=` object keeps its
        // constructor defaults.
        self.data
            .push_error("\"Like\" is not implemented for DynamicExp.");
    }

    fn clone_box(&self) -> Box<dyn DssObject> {
        Box::new(self.clone())
    }
}

/// Delphi/FPC `TStringHelper.Substring(startIndex, length)`: 0-based start,
/// `length` chars, clamped to the string end (`""` when start is past the end).
/// The DynamicExp expressions are ASCII, so byte slicing is char slicing.
fn substring(s: &str, start: usize, length: usize) -> String {
    let bytes = s.as_bytes();
    if start >= bytes.len() {
        return String::new();
    }
    let end = (start + length).min(bytes.len());
    s[start..end].to_string()
}

/// Pascal `InterpretTStringListArray` (the value-list branch): tokenize `s`
/// through a scratch parser into its whitespace/comma-separated values. The
/// DynamicExp call site passes `ApplyLower = False` and the full expression is
/// already lowercased, so no extra lowercasing is applied here. The `file=`
/// branch never applies to an expression sub-string.
fn interpret_string_list(s: &str) -> Vec<String> {
    let mut parser = Parser::new();
    let vars = ParserVars::new();
    parser.set_auto_increment(false);
    parser.set_cmd_string(s);
    let mut list = Vec::new();
    loop {
        parser.next_param(&vars);
        let token = parser.make_string(&vars);
        if token.is_empty() {
            break;
        }
        list.push(token);
    }
    list
}
