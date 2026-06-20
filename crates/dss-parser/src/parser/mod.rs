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
//!
//! Split into submodules (no behavioral change):
//! - `error` — the `ParserError` conversion/inline-math failure type.
//! - `convert` — the FPC `Val`/`Round` number conversions.
//! - `scanner` — the low-level byte scanner (`SkipWhiteSpace`, `IsDelimiter`,
//!   `GetToken`).
//! - `value` — `@variable` substitution, `NextParam`, the `Make*` token
//!   converters and the inline RPN interpreter.
//! - `structures` — `ParseAsBusName`/`ParseAsVector`/`ParseAsMatrix`/
//!   `ParseAsSymMatrix`.

use crate::rpn::RPNCalculator;

mod convert;
mod error;
mod scanner;
mod structures;
mod value;

#[cfg(test)]
mod tests;

pub use convert::{val_f64, val_i32};
pub use error::ParserError;

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
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}
