//! `@variable` substitution, `NextParam`, the `Make*` token converters and the
//! inline RPN interpreter (Pascal `CheckForVar`/`NextParam`/`MakeString`/
//! `MakeInteger`/`MakeDouble`/`InterpretRPNString`/`ProcessRPNCommand`).

use crate::rpn::RPNCalculator;
use crate::vars::ParserVars;

use super::Parser;
use super::convert::{pascal_round_to_i32, val_f64, val_i32};
use super::error::ParserError;

const VARIABLE_DELIMITER: char = '@';

/// Apply one token to the RPN calculator (Pascal `ProcessRPNCommand`):
/// numbers enter the X register, anything else must be a known operation.
fn process_rpn_command(token: &str, rpn: &mut RPNCalculator) -> Result<(), ParserError> {
    if let Some(number) = val_f64(token) {
        rpn.set_x(number);
        return Ok(());
    }
    match token.to_ascii_lowercase().as_str() {
        "+" => rpn.add(),
        "-" => rpn.subtract(),
        "*" => rpn.multiply(),
        "/" => rpn.divide(),
        "sqrt" => rpn.sqrt(),
        "sqr" => rpn.square(),
        "^" => rpn.y_to_the_x_power(),
        "sin" => rpn.sin_deg(),
        "cos" => rpn.cos_deg(),
        "tan" => rpn.tan_deg(),
        "asin" => rpn.asin_deg(),
        "acos" => rpn.acos_deg(),
        "atan" => rpn.atan_deg(),
        "atan2" => rpn.atan2_deg(),
        "swap" => rpn.swap_xy(),
        "rollup" => rpn.roll_up(),
        "rolldn" => rpn.roll_down(),
        "ln" => rpn.nat_log(),
        "pi" => rpn.enter_pi(),
        "log10" => rpn.ten_log(),
        "exp" => rpn.etothex(),
        "inv" => rpn.inv(),
        _ => {
            return Err(ParserError::new(format!(
                "Invalid inline math entry: \"{token}\""
            )));
        }
    }
    Ok(())
}

impl Parser {
    /// Substitute an `@variable` token in place (the parser's own use of
    /// Pascal `CheckForVar`, operating on the current token buffer).
    pub(super) fn check_for_var(&mut self, vars: &ParserVars) -> bool {
        let mut token = std::mem::take(&mut self.token_buffer);
        let changed = self.check_for_var_in(vars, &mut token);
        self.token_buffer = token;
        changed
    }

    /// Substitute an `@variable` in a caller-provided token (the public
    /// Pascal `CheckforVar(var TokenBuffer_)`, used e.g. when resolving
    /// object names). The variable name runs to the first `.` — or `^`,
    /// which takes precedence; the suffix from that character on is kept.
    /// A brace-wrapped value (a definition that itself used variables)
    /// forces RPN interpretation by flagging the parser's quoted state.
    /// Returns whether the token changed.
    pub fn check_for_var_in(&mut self, vars: &ParserVars, token: &mut String) -> bool {
        if token.len() <= 1 || !token.starts_with(VARIABLE_DELIMITER) {
            return false;
        }
        let dot_pos = token.find('.');
        let carat_pos = token.find('^');
        let cut = carat_pos.or(dot_pos); // carat takes precedence

        let var_name = match cut {
            Some(p) => &token[..p],
            None => token.as_str(),
        };
        let Some(value) = vars.get(var_name) else {
            return false;
        };

        let replacement = if value.starts_with('{') {
            // strip the braces added when the variable was defined
            value[1..value.len() - 1].to_string()
        } else {
            value.to_string()
        };
        let force_rpn = value.starts_with('{');

        let new_token = match cut {
            Some(p) => format!("{replacement}{}", &token[p..]),
            None => replacement,
        };
        if force_rpn {
            self.is_quoted_string = true; // force the RPN parser to handle it
        }
        let changed = new_token != *token;
        *token = new_token;
        changed
    }

    /// Advance to the next `name=value` or bare token. Returns the parameter
    /// name (empty when the token had no `name=` part); the token itself is
    /// available via [`Parser::token`] / the `make_*` converters.
    pub fn next_param(&mut self, vars: &ParserVars) -> String {
        if self.position < self.cmd_buffer.len() {
            self.last_delimiter = b' ';
            let buf = std::mem::take(&mut self.cmd_buffer);
            let mut pos = self.position;
            self.token_buffer = self.get_token_at(&buf, &mut pos);
            if self.last_delimiter == b'=' {
                self.parameter_buffer = std::mem::take(&mut self.token_buffer);
                self.token_buffer = self.get_token_at(&buf, &mut pos);
            } else {
                self.parameter_buffer.clear();
            }
            self.position = pos;
            self.cmd_buffer = buf;
        } else {
            self.parameter_buffer.clear();
            self.token_buffer.clear();
        }
        self.check_for_var(vars);
        self.parameter_buffer.clone()
    }

    /// Current token as a string (Pascal `MakeString`/`StrValue`).
    pub fn make_string(&mut self, vars: &ParserVars) -> String {
        if self.auto_increment {
            self.next_param(vars);
        }
        self.token_buffer.clone()
    }

    /// Current token as an integer (Pascal `MakeInteger`/`IntValue`):
    /// integer `Val` (with `$`/`0x`/`%`/`&` prefixes) first, then float
    /// conversion rounded ties-to-even, then error. Empty tokens give 0.
    pub fn make_integer(&mut self, vars: &ParserVars) -> Result<i32, ParserError> {
        self.convert_error = false;
        if self.auto_increment {
            self.next_param(vars);
        }
        if self.token_buffer.is_empty() {
            return Ok(0);
        }
        if self.is_quoted_string {
            let (value, _) = self.interpret_rpn_string(vars)?;
            return Ok(pascal_round_to_i32(value));
        }
        if let Some(value) = val_i32(&self.token_buffer) {
            return Ok(value);
        }
        if let Some(value) = val_f64(&self.token_buffer) {
            return Ok(pascal_round_to_i32(value));
        }
        self.convert_error = true;
        Err(ParserError::new(format!(
            "Integer number conversion error for string: \"{}\"",
            self.token_buffer
        )))
    }

    /// Current token as a double (Pascal `MakeDouble`/`DblValue`). Quoted
    /// tokens (and brace-wrapped variable substitutions) go through the
    /// inline RPN interpreter. Empty tokens give 0.0.
    pub fn make_double(&mut self, vars: &ParserVars) -> Result<f64, ParserError> {
        self.make_double_ex(vars).map(|(v, _)| v)
    }

    /// Like [`Parser::make_double`] but also reports whether the value
    /// actually required RPN evaluation — more than one token inside the
    /// quotes (the Pascal `requiredRPN` out-parameter).
    pub fn make_double_ex(&mut self, vars: &ParserVars) -> Result<(f64, bool), ParserError> {
        if self.auto_increment {
            self.next_param(vars);
        }
        self.convert_error = false;
        if self.token_buffer.is_empty() {
            return Ok((0.0, false));
        }
        if self.is_quoted_string {
            return self.interpret_rpn_string(vars);
        }
        match val_f64(&self.token_buffer) {
            Some(value) => Ok((value, false)),
            None => {
                self.convert_error = true;
                Err(ParserError::new(format!(
                    "Floating point number conversion error for string: \"{}\"",
                    self.token_buffer
                )))
            }
        }
    }

    /// Current token as a complex number `(re, im)` (Pascal
    /// `TDSSParser.MakeComplex`, `ParserDel.pas`). Reads `token_buffer`
    /// directly (no `NextParam` — the vector/matrix parsers set the token
    /// first). A token containing `i`/`I`/`j`/`J` splits at that letter: the
    /// substring before it is read as two whitespace-separated reals `re im`
    /// (FPC `ReadStr(spart, re, im)`); on failure as a single real taken as the
    /// imaginary part (FPC `ReadStr(spart, im)`); on failure `(0, 0)`. Without
    /// an imaginary marker the whole token is the real part (im `= 0`), `(0, 0)`
    /// on a conversion error. Matches capi015 (`5` → `5+0i`, `3i` → `0+3i`,
    /// `5+3i` → `0` since `5+3` is not two whitespace-separated reals).
    pub fn make_complex(&self) -> (f64, f64) {
        let token = self.token_buffer.as_str();
        let ipos = token
            .bytes()
            .position(|b| matches!(b, b'i' | b'I' | b'j' | b'J'));
        if let Some(ipos) = ipos {
            let spart = &token[..ipos];
            let parts: Vec<&str> = spart.split_whitespace().collect();
            // FPC `ReadStr(spart, re, im)`: both reals required.
            if let (Some(re), Some(im)) = (
                parts.first().and_then(|s| val_f64(s)),
                parts.get(1).and_then(|s| val_f64(s)),
            ) {
                return (re, im);
            }
            // FPC `ReadStr(spart, im)`: single leading real → imaginary.
            if let Some(im) = parts.first().and_then(|s| val_f64(s)) {
                return (0.0, im);
            }
            return (0.0, 0.0);
        }
        match val_f64(token) {
            Some(re) => (re, 0.0),
            None => (0.0, 0.0),
        }
    }

    /// Evaluate the current (quoted) token as a whitespace-separated RPN
    /// program; the X register persists across calls, exactly like the
    /// Pascal calculator instance (Pascal `InterpretRPNString`).
    fn interpret_rpn_string(&mut self, vars: &ParserVars) -> Result<(f64, bool), ParserError> {
        let mut cnt = 0u32;
        let parse_buffer = format!("{} ", self.token_buffer);
        let mut pos = 0usize;

        self.skip_white_space(&parse_buffer, &mut pos);
        self.token_buffer = self.get_token_at(&parse_buffer, &mut pos);
        if self.check_for_var(vars) {
            cnt += 1;
        }

        while !self.token_buffer.is_empty() {
            process_rpn_command(&self.token_buffer, &mut self.rpn)?;
            self.token_buffer = self.get_token_at(&parse_buffer, &mut pos);
            self.check_for_var(vars);
            cnt += 1;
        }

        let result = self.rpn.get_x();
        // prepare for the next trip (vector rows continue past RPN entries)
        self.token_buffer = parse_buffer.get(pos..).unwrap_or("").to_string();
        Ok((result, cnt > 1))
    }
}
