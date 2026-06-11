//! The DSS command tokenizer, port of `TDSSParser` from `Parser/ParserDel.pas`.
//!
//! The tokenizer scans bytes like the Pascal original scanned 1-byte chars;
//! delimiter, whitespace and quote sets are compared ASCII-only, so UTF-8
//! content inside tokens passes through untouched. The Pascal loop bounds
//! (`while LinePos < Length(...)`) deliberately never examine the final
//! character — every scanned buffer gets one space appended, exactly like the
//! original — and those off-by-one quirks are reproduced bit-for-bit here.
//!
//! Number conversion mirrors FPC `Val`, verified empirically against the
//! reference engine (see `tools/golden/probe_val.py`): floats accept `.5`,
//! `5.`, `1.e3`, `inf`, `nan` and reject `$FF`/`0xFF`/`1_000`; integers
//! accept `$`/`0x` hex, `%` binary, `&` octal, and fall back to float
//! conversion with banker's rounding (`2.5` → 2, `1.5` → 2).

use crate::rpn::RPNCalculator;
use crate::vars::ParserVars;

const COMMENT_CHAR: u8 = b'!';
const VARIABLE_DELIMITER: char = '@';

/// Conversion or inline-math failure (Pascal `EParserProblem`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserError {
    message: String,
}

impl ParserError {
    /// Build an error carrying a message. Public because the engine crate
    /// reuses this type wherever the Pascal original raised an exception that
    /// `ProcessCommand` would catch (`EParserProblem` and plain `Exception`).
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParserError {}

/// FPC `Val` for doubles. Rust's `f64::from_str` matches it on every probed
/// case except the verbose `infinity` spelling, which is rejected here.
///
/// TODO(compat): the `infinity` rejection only mirrors FPC's narrower
/// grammar; collapse to plain `f64::from_str` once the 1:1 port is complete.
pub fn val_f64(s: &str) -> Option<f64> {
    let t = s.strip_prefix(['+', '-']).unwrap_or(s);
    if t.eq_ignore_ascii_case("infinity") {
        return None;
    }
    s.parse::<f64>().ok()
}

/// FPC `Val` for integers: optional sign, then `$`/`0x`/`0X` hex, `%` binary,
/// `&` octal, or decimal. Out-of-range values fail (range check), letting the
/// caller fall back to float conversion.
pub fn val_i32(s: &str) -> Option<i32> {
    let (neg, rest) = match s.as_bytes().first()? {
        b'+' => (false, &s[1..]),
        b'-' => (true, &s[1..]),
        _ => (false, s),
    };
    let (radix, digits) = if let Some(h) = rest.strip_prefix('$') {
        (16, h)
    } else if let Some(h) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        (16, h)
    } else if let Some(b) = rest.strip_prefix('%') {
        (2, b)
    } else if let Some(o) = rest.strip_prefix('&') {
        (8, o)
    } else {
        (10, rest)
    };
    if digits.is_empty() {
        return None;
    }
    let magnitude = i64::from_str_radix(digits, radix).ok()?;
    let value = if neg { -magnitude } else { magnitude };
    i32::try_from(value).ok()
}

/// FPC `Round`: round-to-nearest-even to Int64 (x87/SSE default mode; out of
/// range and non-finite give the "integer indefinite" `i64::MIN`), then
/// truncated to i32 like the Pascal `Integer := Round(...)` assignment.
///
/// TODO(compat): the integer-indefinite path (`inf`/`nan`/overflow → wrapped
/// `i64::MIN`, e.g. "inf" → 0) reproduces an FPC/x86 implementation artifact
/// verified via probe_val.py; make it a proper error once the 1:1 port is
/// complete.
fn pascal_round_to_i32(x: f64) -> i32 {
    let r = x.round_ties_even();
    let wide = if r >= -(2f64.powi(63)) && r < 2f64.powi(63) {
        r as i64 // r is finite here: NaN comparisons are false
    } else {
        i64::MIN
    };
    wide as i32
}

/// True when `b` occurs in `set`, ASCII-only (a multi-byte char in `set` can
/// never match, just as in the byte-based Pascal scanner).
fn in_set(set: &str, b: u8) -> bool {
    b.is_ascii() && set.as_bytes().contains(&b)
}

/// Apply one token to the RPN calculator (Pascal `ProcessRPNCommand`):
/// numbers enter the X register, anything else must be a known operation.
fn process_rpn_command(token: &str, rpn: &mut RPNCalculator) -> Result<(), ParserError> {
    if let Some(number) = val_f64(token) {
        rpn.set_x(number);
        return Ok(());
    }
    match token.to_lowercase().as_str() {
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

/// The DSS command tokenizer (Pascal `TDSSParser`).
///
/// Methods that can substitute `@variables` or evaluate inline RPN take the
/// shared variable table explicitly; in the engine that table lives on the
/// DSS context and is shared by the main, auxiliary, and property parsers.
#[derive(Debug)]
pub struct Parser {
    cmd_buffer: String,
    /// 0-based byte offset into `cmd_buffer`.
    position: usize,
    parameter_buffer: String,
    token_buffer: String,
    delim_chars: String,
    white_space_chars: String,
    begin_quote_chars: String,
    end_quote_chars: String,
    last_delimiter: u8,
    matrix_row_terminator: u8,
    auto_increment: bool,
    convert_error: bool,
    is_quoted_string: bool,
    rpn: RPNCalculator,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            cmd_buffer: String::new(),
            position: 0,
            parameter_buffer: String::new(),
            token_buffer: String::new(),
            delim_chars: ",=".to_string(),
            white_space_chars: " \t".to_string(),
            begin_quote_chars: "(\"'[{".to_string(),
            end_quote_chars: ")\"']}".to_string(),
            last_delimiter: b' ',
            matrix_row_terminator: b'|',
            auto_increment: false,
            convert_error: false,
            is_quoted_string: false,
            rpn: RPNCalculator::new(),
        }
    }

    /// Load a command line; a trailing space is appended so the last token
    /// terminates, and leading whitespace is skipped (Pascal `SetCmdString`).
    pub fn set_cmd_string(&mut self, value: &str) {
        self.cmd_buffer = format!("{value} ");
        let mut pos = 0;
        self.skip_white_space(&self.cmd_buffer, &mut pos);
        self.position = pos; // first non-whitespace character
    }

    /// The full stored command line, including the appended trailing space.
    pub fn cmd_string(&self) -> &str {
        &self.cmd_buffer
    }

    /// Restore the default delimiters, whitespace, quote pairs, and matrix
    /// row terminator (Pascal `ResetDelims`).
    pub fn reset_delims(&mut self) {
        self.delim_chars = ",=".to_string();
        self.white_space_chars = " \t".to_string();
        self.matrix_row_terminator = b'|';
        self.begin_quote_chars = "(\"'[{".to_string();
        self.end_quote_chars = ")\"']}".to_string();
    }

    pub fn delimiters(&self) -> &str {
        &self.delim_chars
    }

    pub fn set_delimiters(&mut self, delims: &str) {
        self.delim_chars = delims.to_string();
    }

    pub fn whitespace(&self) -> &str {
        &self.white_space_chars
    }

    pub fn set_whitespace(&mut self, ws: &str) {
        self.white_space_chars = ws.to_string();
    }

    pub fn begin_quote_chars(&self) -> &str {
        &self.begin_quote_chars
    }

    pub fn set_begin_quote_chars(&mut self, chars: &str) {
        self.begin_quote_chars = chars.to_string();
    }

    pub fn end_quote_chars(&self) -> &str {
        &self.end_quote_chars
    }

    pub fn set_end_quote_chars(&mut self, chars: &str) {
        self.end_quote_chars = chars.to_string();
    }

    pub fn auto_increment(&self) -> bool {
        self.auto_increment
    }

    pub fn set_auto_increment(&mut self, on: bool) {
        self.auto_increment = on;
    }

    /// Current token (Pascal `Token` property read).
    pub fn token(&self) -> &str {
        &self.token_buffer
    }

    /// Replace the current token (Pascal `Token` property write).
    pub fn set_token(&mut self, token: &str) {
        self.token_buffer = token.to_string();
    }

    /// Scan position, for save/restore (Pascal `Position`); a 0-based byte
    /// offset where the Pascal property was 1-based.
    pub fn position(&self) -> usize {
        self.position
    }

    pub fn set_position(&mut self, pos: usize) {
        self.position = pos;
    }

    /// Unscanned rest of the command line (Pascal `Remainder`).
    pub fn remainder(&self) -> &str {
        self.cmd_buffer.get(self.position..).unwrap_or("")
    }

    /// True when the last `make_integer`/`make_double` hit a conversion
    /// error (Pascal `ConvertError`; also signalled by the returned `Err`).
    pub fn convert_error(&self) -> bool {
        self.convert_error
    }

    fn is_white_space(&self, b: u8) -> bool {
        in_set(&self.white_space_chars, b)
    }

    fn is_delim_char(&self, b: u8) -> bool {
        in_set(&self.delim_chars, b)
    }

    /// Pascal `SkipWhiteSpace`: never examines the last character of the
    /// buffer (the appended space).
    fn skip_white_space(&self, buf: &str, pos: &mut usize) {
        let bytes = buf.as_bytes();
        while *pos + 1 < bytes.len() && self.is_white_space(bytes[*pos]) {
            *pos += 1;
        }
    }

    /// Pascal `IsCommentChar`: `!` anywhere, or `//` (needs a next char).
    fn is_comment_char(buf: &[u8], pos: usize) -> bool {
        match buf[pos] {
            COMMENT_CHAR => true,
            b'/' => pos + 1 < buf.len() && buf[pos + 1] == b'/',
            _ => false,
        }
    }

    /// Pascal `IsDelimiter`: comment, delimiter char, or whitespace; records
    /// what stopped the scan in `last_delimiter` (whitespace records `' '`).
    fn is_delimiter(&mut self, buf: &[u8], pos: usize) -> bool {
        if Self::is_comment_char(buf, pos) {
            self.last_delimiter = COMMENT_CHAR;
            return true;
        }
        let ch = buf[pos];
        if self.is_delim_char(ch) {
            self.last_delimiter = ch;
            return true;
        }
        if self.is_white_space(ch) {
            self.last_delimiter = b' ';
            return true;
        }
        false
    }

    /// Pascal `GetToken`, the scanner core. Operates on any buffer (the
    /// command line, a vector/matrix sub-buffer, a bus-name node list...);
    /// the buffer must carry the customary appended trailing space.
    fn get_token_at(&mut self, buf: &str, pos: &mut usize) -> String {
        let bytes = buf.as_bytes();
        let len = bytes.len();
        let mut result = String::new();

        if *pos < len {
            self.is_quoted_string = false;
            let quote_index = if bytes[*pos].is_ascii() {
                self.begin_quote_chars
                    .as_bytes()
                    .iter()
                    .position(|&q| q == bytes[*pos])
            } else {
                None
            };

            let matched_end = quote_index.and_then(|qi| self.end_quote_chars.as_bytes().get(qi));
            if let Some(&end_char) = matched_end {
                // ParseToEndQuote / ParseToEndChar
                *pos += 1;
                let start = *pos;
                while *pos + 1 < len && bytes[*pos] != end_char {
                    *pos += 1;
                }
                result = buf[start..*pos].to_string();
                if *pos + 1 < len {
                    *pos += 1; // move past the end quote
                }
                self.is_quoted_string = true;
            } else {
                let start = *pos;
                while *pos + 1 < len && !self.is_delimiter(bytes, *pos) {
                    *pos += 1;
                }
                result = buf[start..*pos].to_string();
            }

            if self.last_delimiter == COMMENT_CHAR {
                // stop on comment: ignore the rest of the line
                *pos = len;
            } else {
                if self.last_delimiter == b' ' {
                    self.skip_white_space(buf, pos);
                }
                if self.is_delim_char(bytes[*pos]) {
                    self.last_delimiter = bytes[*pos];
                    *pos += 1; // move past the terminating delimiter
                }
                self.skip_white_space(buf, pos);
            }
        }
        result
    }

    /// Substitute an `@variable` token in place (the parser's own use of
    /// Pascal `CheckForVar`, operating on the current token buffer).
    fn check_for_var(&mut self, vars: &ParserVars) -> bool {
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

    /// Split `"busname.1.2.3"` into the bus name and its node numbers
    /// (Pascal `ParseAsBusName`). Without a dot the whole token is the name
    /// (untrimmed, like the original); with nodes the name is trimmed.
    pub fn parse_as_bus_name(
        &mut self,
        param: &str,
        vars: &ParserVars,
    ) -> Result<(String, Vec<i32>), ParserError> {
        self.token_buffer = param.to_string();
        if self.auto_increment {
            self.next_param(vars);
        }
        let Some(dot_pos) = self.token_buffer.find('.') else {
            return Ok((self.token_buffer.clone(), Vec::new()));
        };
        let name = self.token_buffer[..dot_pos].trim().to_string();
        let token_save = std::mem::take(&mut self.token_buffer);
        let node_buffer = format!("{} ", &token_save[dot_pos + 1..]);

        let delim_save = std::mem::replace(&mut self.delim_chars, ".".to_string());
        let mut nodes = Vec::new();
        let mut pos = 0usize;
        let mut error = None;

        self.token_buffer = self.get_token_at(&node_buffer, &mut pos);
        while !self.token_buffer.is_empty() {
            match self.make_integer(vars) {
                Ok(v) => nodes.push(if self.convert_error { -1 } else { v }),
                Err(e) => {
                    error = Some(e);
                    break;
                }
            }
            self.token_buffer = self.get_token_at(&node_buffer, &mut pos);
        }

        self.delim_chars = delim_save; // restore original delimiters
        self.token_buffer = token_save;
        match error {
            Some(e) => Err(e),
            None => Ok((name, nodes)),
        }
    }

    /// Parse the current token as a vector of doubles into `out`
    /// (Pascal `ParseAsVector`). Returns the number of elements *found* —
    /// which may exceed `out.len()`; the extras are consumed but dropped.
    /// Scanning stops at the matrix row terminator `|`, leaving the rest of
    /// the token for the next row. With `do_round` each stored element is
    /// rounded ties-to-even (Pascal `DoRound`).
    pub fn parse_as_vector(
        &mut self,
        vars: &ParserVars,
        out: &mut [f64],
        do_round: bool,
    ) -> Result<usize, ParserError> {
        if self.auto_increment {
            self.next_param(vars);
        }
        let mut num_elements = 0usize;
        out.fill(0.0);

        let parse_buffer = format!("{} ", self.token_buffer);
        let mut pos = 0usize;
        let delim_save = self.delim_chars.clone();
        self.delim_chars.push(self.matrix_row_terminator as char);
        let mut error = None;

        self.skip_white_space(&parse_buffer, &mut pos);
        self.token_buffer = self.get_token_at(&parse_buffer, &mut pos);
        self.check_for_var(vars);
        while !self.token_buffer.is_empty() {
            num_elements += 1;
            if num_elements <= out.len() {
                match self.make_double(vars) {
                    Ok(v) => out[num_elements - 1] = v,
                    Err(e) => {
                        error = Some(e);
                        break;
                    }
                }
            }
            if self.last_delimiter == self.matrix_row_terminator {
                break;
            }
            self.token_buffer = self.get_token_at(&parse_buffer, &mut pos);
            self.check_for_var(vars);
        }

        self.delim_chars = delim_save; // restore original delimiters
        // prepare for the next trip (the following matrix row)
        self.token_buffer = parse_buffer.get(pos..).unwrap_or("").to_string();
        if do_round {
            let stored = num_elements.min(out.len());
            for v in out[..stored].iter_mut() {
                *v = v.round_ties_even();
            }
        }
        match error {
            Some(e) => Err(e),
            None => Ok(num_elements),
        }
    }

    /// Parse `order` rows separated by `|` into a full matrix in
    /// column-major (Fortran) order (Pascal `ParseAsMatrix`).
    /// Returns `order` on success.
    pub fn parse_as_matrix(
        &mut self,
        vars: &ParserVars,
        out: &mut [f64],
        order: usize,
    ) -> Result<usize, ParserError> {
        if self.auto_increment {
            self.next_param(vars);
        }
        let mut row_buf = vec![0.0; order];
        out[..order * order].fill(0.0);

        for i in 0..order {
            let elements_found = self.parse_as_vector(vars, &mut row_buf, false)?;
            if elements_found > order * order {
                return Err(ParserError::new(
                    "Matrix Buffer in ParseAsMatrix too small. Check your input data, \
                     especially dimensions and number of phases."
                        .to_string(),
                ));
            }
            // Pascal read past RowBuf for elements beyond the order
            // (undefined behavior there); the extras are ignored here.
            for j in 0..elements_found.min(order) {
                out[j * order + i] = row_buf[j];
            }
        }
        Ok(order)
    }

    /// Parse a lower-triangle-by-rows symmetric matrix into a full
    /// column-major matrix with optional element `stride` and `scale`
    /// (Pascal `ParseAsSymMatrix`). Returns `order` on success.
    pub fn parse_as_sym_matrix(
        &mut self,
        vars: &ParserVars,
        out: &mut [f64],
        order: usize,
        stride: usize,
        scale: f64,
    ) -> Result<usize, ParserError> {
        if self.auto_increment {
            self.next_param(vars);
        }
        let mut row_buf = vec![0.0; order];
        let maxpos = order * order - 1;
        for i in 0..order * order {
            out[i * stride] = 0.0;
        }

        for i in 0..order {
            let elements_found = self.parse_as_vector(vars, &mut row_buf, false)?;
            // A range loop on purpose: when a row has more elements than the
            // order, the subpos check below must error out BEFORE row_buf[j]
            // is read (the Pascal code's exact behavior); an iterator would
            // silently stop at the buffer end instead.
            #[allow(clippy::needless_range_loop)]
            for j in 0..elements_found {
                let subpos = j * order + i;
                if subpos > maxpos {
                    return Err(ParserError::new(
                        "Matrix Buffer in ParseAsSymMatrix too small. Check your input \
                         data, especially dimensions and number of phases."
                            .to_string(),
                    ));
                }
                out[subpos * stride] = row_buf[j] * scale;

                if i == j {
                    continue;
                }
                let subpos = i * order + j;
                if subpos > maxpos {
                    return Err(ParserError::new(
                        "Matrix Buffer in ParseAsSymMatrix too small. Check your input \
                         data, especially dimensions and number of phases."
                            .to_string(),
                    ));
                }
                out[subpos * stride] = row_buf[j] * scale;
            }
        }
        Ok(order)
    }
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parser_with(cmd: &str) -> (Parser, ParserVars) {
        let mut p = Parser::new();
        p.set_cmd_string(cmd);
        (p, ParserVars::new())
    }

    #[test]
    fn next_param_name_value_pairs() {
        let (mut p, vars) = parser_with("param1=value1 param2=value2");
        assert_eq!(p.next_param(&vars), "param1");
        assert_eq!(p.token(), "value1");
        assert_eq!(p.next_param(&vars), "param2");
        assert_eq!(p.token(), "value2");
        assert_eq!(p.next_param(&vars), "");
        assert_eq!(p.token(), "");
    }

    #[test]
    fn equals_with_surrounding_spaces() {
        let (mut p, vars) = parser_with("kv = 12.47");
        assert_eq!(p.next_param(&vars), "kv");
        assert_eq!(p.token(), "12.47");
    }

    #[test]
    fn positional_tokens_have_empty_param_names() {
        let (mut p, vars) = parser_with("New Line.L1 Bus1 Bus2");
        assert_eq!(p.next_param(&vars), "");
        assert_eq!(p.token(), "New");
        assert_eq!(p.next_param(&vars), "");
        assert_eq!(p.token(), "Line.L1");
        p.next_param(&vars);
        assert_eq!(p.token(), "Bus1");
    }

    #[test]
    fn commas_are_delimiters() {
        let (mut p, vars) = parser_with("a,b, c");
        p.next_param(&vars);
        assert_eq!(p.token(), "a");
        p.next_param(&vars);
        assert_eq!(p.token(), "b");
        p.next_param(&vars);
        assert_eq!(p.token(), "c");
    }

    #[test]
    fn all_quote_pairs() {
        for (open, close) in [('(', ')'), ('"', '"'), ('\'', '\''), ('[', ']'), ('{', '}')] {
            let (mut p, vars) = parser_with(&format!("x={open}alpha beta{close} next"));
            assert_eq!(p.next_param(&vars), "x");
            assert_eq!(p.token(), "alpha beta", "quote pair {open}{close}");
            p.next_param(&vars);
            assert_eq!(p.token(), "next");
        }
    }

    #[test]
    fn bang_comment_stops_the_line() {
        let (mut p, vars) = parser_with("kw=5 ! the rest is comment kv=12");
        assert_eq!(p.next_param(&vars), "kw");
        assert_eq!(p.token(), "5");
        assert_eq!(p.next_param(&vars), "");
        assert_eq!(p.token(), "");
    }

    #[test]
    fn double_slash_comment_stops_the_line() {
        let (mut p, vars) = parser_with("conn=delta // trailing comment");
        assert_eq!(p.next_param(&vars), "conn");
        assert_eq!(p.token(), "delta");
        p.next_param(&vars);
        assert_eq!(p.token(), "");
    }

    #[test]
    fn slash_inside_token_is_not_a_comment() {
        let (mut p, vars) = parser_with("file=dir/sub/name.dss");
        assert_eq!(p.next_param(&vars), "file");
        assert_eq!(p.token(), "dir/sub/name.dss");
    }

    #[test]
    fn make_double_accepts_fpc_val_forms() {
        // Forms verified against the reference parser (probe_val.py).
        for (s, expected) in [
            (".5", 0.5),
            ("5.", 5.0),
            ("1e3", 1000.0),
            ("1E3", 1000.0),
            ("+5", 5.0),
            ("-0.25", -0.25),
            ("1.5e-3", 0.0015),
            ("007", 7.0),
            ("-.5", -0.5),
            ("1e+3", 1000.0),
            ("1.e3", 1000.0),
        ] {
            let (mut p, vars) = parser_with(&format!("x={s}"));
            p.next_param(&vars);
            assert_eq!(p.make_double(&vars).unwrap(), expected, "input {s:?}");
        }
        // inf/nan are accepted by FPC Val
        let (mut p, vars) = parser_with("x=inf");
        p.next_param(&vars);
        assert_eq!(p.make_double(&vars).unwrap(), f64::INFINITY);
        let (mut p, vars) = parser_with("x=nan");
        p.next_param(&vars);
        assert!(p.make_double(&vars).unwrap().is_nan());
    }

    #[test]
    fn make_double_rejects_what_fpc_rejects() {
        for s in ["$FF", "0xFF", "1_000", "5.5.5", "1e", "abc", "infinity"] {
            let (mut p, vars) = parser_with(&format!("x={s}"));
            p.next_param(&vars);
            assert!(p.make_double(&vars).is_err(), "input {s:?}");
            assert!(p.convert_error(), "input {s:?}");
        }
    }

    #[test]
    fn make_double_empty_token_is_zero() {
        let (mut p, vars) = parser_with("");
        p.next_param(&vars);
        assert_eq!(p.make_double(&vars).unwrap(), 0.0);
        assert!(!p.convert_error());
    }

    #[test]
    fn make_integer_radix_prefixes() {
        // Verified against the reference parser (probe_val.py).
        for (s, expected) in [
            ("$FF", 255),
            ("$ff", 255),
            ("0xFF", 255),
            ("%101", 5),
            ("&777", 511),
            ("007", 7),
            ("+5", 5),
            ("1e3", 1000),
        ] {
            let (mut p, vars) = parser_with(&format!("n={s}"));
            p.next_param(&vars);
            assert_eq!(p.make_integer(&vars).unwrap(), expected, "input {s:?}");
        }
    }

    #[test]
    fn make_integer_rounds_ties_to_even() {
        // FPC Round is banker's rounding: 1.5 → 2, 2.5 → 2, .5 → 0.
        for (s, expected) in [("1.5", 2), ("2.5", 2), (".5", 0), ("-.5", 0), ("-0.25", 0)] {
            let (mut p, vars) = parser_with(&format!("n={s}"));
            p.next_param(&vars);
            assert_eq!(p.make_integer(&vars).unwrap(), expected, "input {s:?}");
        }
        // non-finite values collapse to 0 through the integer-indefinite path
        for s in ["inf", "nan"] {
            let (mut p, vars) = parser_with(&format!("n={s}"));
            p.next_param(&vars);
            assert_eq!(p.make_integer(&vars).unwrap(), 0, "input {s:?}");
        }
    }

    #[test]
    fn make_integer_errors_match_fpc() {
        for s in ["1_000", "5.5.5", "1e", "abc"] {
            let (mut p, vars) = parser_with(&format!("n={s}"));
            p.next_param(&vars);
            assert!(p.make_integer(&vars).is_err(), "input {s:?}");
        }
    }

    #[test]
    fn quoted_token_evaluates_as_rpn() {
        let (mut p, vars) = parser_with("x=(1 2 +)");
        p.next_param(&vars);
        let (v, required) = p.make_double_ex(&vars).unwrap();
        assert_eq!(v, 3.0);
        assert!(required);
    }

    #[test]
    fn quoted_single_number_is_not_required_rpn() {
        let (mut p, vars) = parser_with("x=(42.5)");
        p.next_param(&vars);
        let (v, required) = p.make_double_ex(&vars).unwrap();
        assert_eq!(v, 42.5);
        assert!(!required);
    }

    #[test]
    fn rpn_operations_and_functions() {
        for (expr, expected) in [
            ("(2 3 *)", 6.0),
            ("(10 4 -)", 6.0),
            ("(3 4 /)", 0.75),
            ("(2 10 ^)", 1024.0),
            ("(9 sqrt)", 3.0),
            ("(3 sqr)", 9.0),
            ("(0.5 inv)", 2.0),
            ("(10 exp ln)", 10.0),
            ("(1000 log10)", 3.0),
        ] {
            let (mut p, vars) = parser_with(&format!("x={expr}"));
            p.next_param(&vars);
            let v = p.make_double(&vars).unwrap();
            assert!(
                (v - expected).abs() < 1e-12,
                "{expr} → {v}, want {expected}"
            );
        }
        // trig in degrees
        let (mut p, vars) = parser_with("x=(30 sin)");
        p.next_param(&vars);
        assert!((p.make_double(&vars).unwrap() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn rpn_invalid_entry_errors() {
        let (mut p, vars) = parser_with("x=(1 2 bogus)");
        p.next_param(&vars);
        let err = p.make_double(&vars).unwrap_err();
        assert!(err.message().contains("Invalid inline math entry"));
    }

    #[test]
    fn rpn_x_register_persists_between_calls() {
        // The Pascal calculator instance lives on the parser; its stack is
        // never reset between commands.
        let (mut p, vars) = parser_with("a=(10) b=(2 +)");
        p.next_param(&vars);
        assert_eq!(p.make_double(&vars).unwrap(), 10.0);
        p.next_param(&vars);
        assert_eq!(p.make_double(&vars).unwrap(), 12.0);
    }

    #[test]
    fn variable_substitution_plain() {
        let (mut p, mut vars) = parser_with("kv=@v");
        vars.add("@v", "12.47");
        p.next_param(&vars);
        assert_eq!(p.token(), "12.47");
        assert_eq!(p.make_double(&vars).unwrap(), 12.47);
    }

    #[test]
    fn variable_substitution_keeps_dot_suffix() {
        let (mut p, mut vars) = parser_with("bus1=@mybus.1.2.3");
        vars.add("@mybus", "alpha");
        p.next_param(&vars);
        assert_eq!(p.token(), "alpha.1.2.3");
        let (name, nodes) = p.parse_as_bus_name("alpha.1.2.3", &vars).unwrap();
        assert_eq!(name, "alpha");
        assert_eq!(nodes, vec![1, 2, 3]);
    }

    #[test]
    fn variable_defined_with_variables_forces_rpn() {
        let (mut p, mut vars) = parser_with("x=@b");
        vars.add("@a", "2");
        vars.add("@b", "@a 3 *"); // stored as {@a 3 *}
        p.next_param(&vars);
        assert_eq!(p.make_double(&vars).unwrap(), 6.0);
    }

    #[test]
    fn unknown_variable_is_left_alone() {
        let (mut p, vars) = parser_with("x=@nosuchvar");
        p.next_param(&vars);
        assert_eq!(p.token(), "@nosuchvar");
    }

    #[test]
    fn bus_name_parsing() {
        let (mut p, vars) = parser_with("");
        let (name, nodes) = p.parse_as_bus_name("Bus1.1.2.3", &vars).unwrap();
        assert_eq!(name, "Bus1");
        assert_eq!(nodes, vec![1, 2, 3]);

        let (name, nodes) = p.parse_as_bus_name("SourceBus", &vars).unwrap();
        assert_eq!(name, "SourceBus");
        assert!(nodes.is_empty());

        // explicit ground connections
        let (name, nodes) = p.parse_as_bus_name("b2.1.0.0", &vars).unwrap();
        assert_eq!(name, "b2");
        assert_eq!(nodes, vec![1, 0, 0]);
    }

    #[test]
    fn bus_name_restores_parser_state() {
        let (mut p, vars) = parser_with("next=42");
        p.parse_as_bus_name("Bus1.1.2", &vars).unwrap();
        // delimiters were restored: normal parsing continues to work
        assert_eq!(p.next_param(&vars), "next");
        assert_eq!(p.make_integer(&vars).unwrap(), 42);
    }

    #[test]
    fn vector_parsing_from_bracket_token() {
        let (mut p, vars) = parser_with("kvs=[1.0 2.5 3.5]");
        p.next_param(&vars);
        let mut out = [0.0; 3];
        let n = p.parse_as_vector(&vars, &mut out, false).unwrap();
        assert_eq!(n, 3);
        assert_eq!(out, [1.0, 2.5, 3.5]);
    }

    #[test]
    fn vector_counts_extras_but_drops_them() {
        let (mut p, vars) = parser_with("v=[1 2 3 4 5]");
        p.next_param(&vars);
        let mut out = [0.0; 3];
        let n = p.parse_as_vector(&vars, &mut out, false).unwrap();
        assert_eq!(n, 5); // found 5, stored 3 — Pascal reports the count found
        assert_eq!(out, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn vector_with_commas_and_rounding() {
        let (mut p, vars) = parser_with("v=(1.4, 2.5, 3.6)");
        p.next_param(&vars);
        let mut out = [0.0; 3];
        let n = p.parse_as_vector(&vars, &mut out, true).unwrap();
        assert_eq!(n, 3);
        assert_eq!(out, [1.0, 2.0, 4.0]); // 2.5 rounds to even
    }

    #[test]
    fn matrix_rows_are_stored_column_major() {
        let (mut p, vars) = parser_with("m=[1 2 | 3 4]");
        p.next_param(&vars);
        let mut out = [0.0; 4];
        let n = p.parse_as_matrix(&vars, &mut out, 2).unwrap();
        assert_eq!(n, 2);
        // column-major: [a11 a21 a12 a22]
        assert_eq!(out, [1.0, 3.0, 2.0, 4.0]);
    }

    #[test]
    fn sym_matrix_lower_triangle_fills_both_sides() {
        let (mut p, vars) = parser_with("z=[2 | -1 3]");
        p.next_param(&vars);
        let mut out = [0.0; 4];
        let n = p.parse_as_sym_matrix(&vars, &mut out, 2, 1, 1.0).unwrap();
        assert_eq!(n, 2);
        assert_eq!(out, [2.0, -1.0, -1.0, 3.0]);
    }

    #[test]
    fn sym_matrix_with_stride_and_scale() {
        // stride 2 leaves gaps (the engine interleaves re/im parts)
        let (mut p, vars) = parser_with("z=[1 | 2 4]");
        p.next_param(&vars);
        let mut out = [9.0; 8];
        p.parse_as_sym_matrix(&vars, &mut out, 2, 2, 10.0).unwrap();
        assert_eq!(out, [10.0, 9.0, 20.0, 9.0, 20.0, 9.0, 40.0, 9.0]);
    }

    #[test]
    fn matrix_full_rows_no_terminator() {
        // 3x3 matrix as three rows
        let (mut p, vars) = parser_with("m=[1 2 3 | 4 5 6 | 7 8 9]");
        p.next_param(&vars);
        let mut out = [0.0; 9];
        p.parse_as_matrix(&vars, &mut out, 3).unwrap();
        // column-major check of a couple of entries: a21 = 4 at index 1,
        // a13 = 3 at index 6
        assert_eq!(out[1], 4.0);
        assert_eq!(out[6], 3.0);
        assert_eq!(out[8], 9.0);
    }

    #[test]
    fn remainder_and_position_roundtrip() {
        let (mut p, vars) = parser_with("a=1 b=2 c=3");
        p.next_param(&vars);
        let saved = p.position();
        let rem_before = p.remainder().to_string();
        p.next_param(&vars);
        assert_ne!(p.remainder(), rem_before);
        p.set_position(saved);
        assert_eq!(p.remainder(), rem_before);
        assert_eq!(p.next_param(&vars), "b");
    }

    #[test]
    fn reset_delims_restores_defaults() {
        let mut p = Parser::new();
        p.set_delimiters(";");
        p.set_whitespace(" ");
        p.set_begin_quote_chars("<");
        p.set_end_quote_chars(">");
        p.reset_delims();
        assert_eq!(p.delimiters(), ",=");
        assert_eq!(p.whitespace(), " \t");
        assert_eq!(p.begin_quote_chars(), "(\"'[{");
        assert_eq!(p.end_quote_chars(), ")\"']}");
    }

    #[test]
    fn custom_delimiters() {
        let mut p = Parser::new();
        let vars = ParserVars::new();
        p.set_delimiters(",=;");
        p.set_cmd_string("a;b;c");
        p.next_param(&vars);
        assert_eq!(p.token(), "a");
        p.next_param(&vars);
        assert_eq!(p.token(), "b");
        p.next_param(&vars);
        assert_eq!(p.token(), "c");
    }

    #[test]
    fn auto_increment_advances_in_make_string() {
        let (mut p, vars) = parser_with("one two three");
        p.set_auto_increment(true);
        assert_eq!(p.make_string(&vars), "one");
        assert_eq!(p.make_string(&vars), "two");
        assert_eq!(p.make_string(&vars), "three");
    }

    #[test]
    fn tab_is_whitespace() {
        let (mut p, vars) = parser_with("a\tb");
        p.next_param(&vars);
        assert_eq!(p.token(), "a");
        p.next_param(&vars);
        assert_eq!(p.token(), "b");
    }

    #[test]
    fn utf8_content_passes_through_tokens() {
        let (mut p, vars) = parser_with("file=\"путь/к файлу.dss\" kv=12");
        assert_eq!(p.next_param(&vars), "file");
        assert_eq!(p.token(), "путь/к файлу.dss");
        assert_eq!(p.next_param(&vars), "kv");
        assert_eq!(p.make_double(&vars).unwrap(), 12.0);
    }

    #[test]
    fn unterminated_quote_runs_to_end_of_line() {
        let (mut p, vars) = parser_with("x=(1 2");
        p.next_param(&vars);
        // Pascal scans to the end (the appended space is the last char)
        assert_eq!(p.token(), "1 2");
    }
}
