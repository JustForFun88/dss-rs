//! `UserData=` string scanner — a minimal hand-rolled port of the parser
//! subset the vendored DLL actually uses (plan §2.6 sanctions a minimal
//! scanner, no dss-parser dependency). Sources: the DLL-private
//! `IndMach012a/ParserDel.pas` (`TParser`: `SetCmdString`, `GetToken`,
//! `GetNextParam`, `MakeString`, `MakeDouble`) and `Shared/Command.pas` +
//! `Shared/HashList.pas` (`TCommandList.GetCommand`: case-insensitive exact
//! match, then first-prefix `FindAbbrev`).
//!
//! Deliberate fixture-scope reductions (loud, never silent):
//! - RPN inline math inside quoted values (`InterpretRPNString`,
//!   `ParserDel.pas:627-653`) is reduced to "a quoted plain number"; a real
//!   RPN expression panics (wasm trap) naming the token. The reference decks
//!   never use RPN in `UserData=`.
//! - A numeric conversion error, which upstream raises as `EParserProblem`
//!   escaping the Stdcall boundary (undefined behavior / crash), panics here
//!   (deterministic trap) — same failure class, defined outcome.

/// Pascal `TParser` delimiters (`ParserDel.pas:141-153`).
const DELIM_CHARS: &[u8] = b",=";
const WHITESPACE_CHARS: &[u8] = b" \t";
const BEGIN_QUOTE_CHARS: &[u8] = b"(\"'[{";
const END_QUOTE_CHARS: &[u8] = b")\"']}";
const COMMENT_CHAR: u8 = b'!';

/// Port of `TParser` restricted to the members `MainUnit.Edit` exercises.
/// Byte-oriented (strings crossing the ABI are ANSI); `pos` is 1-based to
/// keep the Pascal index arithmetic literal.
pub struct Parser {
    cmd: Vec<u8>,
    pos: usize,
    token: Vec<u8>,
    param: Vec<u8>,
    last_delim: u8,
    is_quoted: bool,
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser {
    pub fn new() -> Self {
        Parser {
            cmd: Vec::new(),
            pos: 1,
            token: Vec::new(),
            param: Vec::new(),
            last_delim: b' ',
            is_quoted: false,
        }
    }

    /// Pascal `TParser.SetCmdString` (`ParserDel.pas:166-171`).
    pub fn set_cmd_string(&mut self, value: &[u8]) {
        self.cmd = value.to_vec();
        self.cmd.push(b' '); // "add some white space at end to get last param"
        self.pos = 1;
        self.skip_white_space();
    }

    fn is_white_space(ch: u8) -> bool {
        WHITESPACE_CHARS.contains(&ch)
    }

    fn is_delim_char(ch: u8) -> bool {
        DELIM_CHARS.contains(&ch)
    }

    /// Pascal `TParser.SkipWhiteSpace` (`ParserDel.pas:252-256`) — note the
    /// `LinePos < Length` bound: never advances past the final character.
    fn skip_white_space(&mut self) {
        while self.pos < self.cmd.len() && Self::is_white_space(self.cmd[self.pos - 1]) {
            self.pos += 1;
        }
    }

    /// Pascal `TParser.IsCommentChar` (`ParserDel.pas:607-623`): `!` or `//`.
    fn is_comment_char(&self) -> bool {
        match self.cmd[self.pos - 1] {
            COMMENT_CHAR => true,
            b'/' => self.cmd.len() > self.pos && self.cmd[self.pos] == b'/',
            _ => false,
        }
    }

    /// Pascal `TParser.IsDelimiter` (`ParserDel.pas:201-233`) — side effect:
    /// records `LastDelimiter` (whitespace collapses to `' '`).
    fn is_delimiter(&mut self) -> bool {
        if self.is_comment_char() {
            self.last_delim = COMMENT_CHAR;
            return true;
        }
        let ch = self.cmd[self.pos - 1];
        if Self::is_delim_char(ch) {
            self.last_delim = ch;
            return true;
        }
        if Self::is_white_space(ch) {
            self.last_delim = b' ';
            return true;
        }
        false
    }

    /// Pascal `TParser.GetToken` (`ParserDel.pas:260-328`), specialized to
    /// `CmdBuffer`/`FPosition` (the only buffer `Edit` parses).
    fn get_token(&mut self) -> Vec<u8> {
        let mut result: Vec<u8> = Vec::new();
        let cmd_buf_length = self.cmd.len();
        if self.pos <= cmd_buf_length {
            self.is_quoted = false;
            let quote_index = BEGIN_QUOTE_CHARS
                .iter()
                .position(|&c| c == self.cmd[self.pos - 1]);
            if let Some(qi) = quote_index {
                // ParseToEndQuote -> ParseToEndChar(FEndQuoteChars[QuoteIndex])
                let end_char = END_QUOTE_CHARS[qi];
                self.pos += 1;
                let token_start = self.pos;
                while self.pos < cmd_buf_length && self.cmd[self.pos - 1] != end_char {
                    self.pos += 1;
                }
                result = self.cmd[token_start - 1..self.pos - 1].to_vec();
                if self.pos < cmd_buf_length {
                    self.pos += 1; // increment past endchar
                }
                self.is_quoted = true;
            } else {
                let token_start = self.pos;
                while self.pos < cmd_buf_length && !self.is_delimiter() {
                    self.pos += 1;
                }
                result = self.cmd[token_start - 1..self.pos - 1].to_vec();
            }

            if self.last_delim == COMMENT_CHAR {
                // stop on comment: ignore rest of line
                self.pos = cmd_buf_length + 1;
            } else {
                if self.last_delim == b' ' {
                    self.skip_white_space();
                }
                if Self::is_delim_char(self.cmd[self.pos - 1]) {
                    self.last_delim = self.cmd[self.pos - 1];
                    self.pos += 1; // move past terminating delimiter
                }
                self.skip_white_space();
            }
        }
        result
    }

    /// Pascal `TParser.GetNextParam` (`ParserDel.pas:333-354`): returns the
    /// parameter *name* (empty for a positional value); the value token stays
    /// in the token buffer for `StrValue`/`DblValue`.
    pub fn next_param(&mut self) -> String {
        if self.pos <= self.cmd.len() {
            self.last_delim = b' ';
            self.token = self.get_token();
            if self.last_delim == b'=' {
                self.param = self.token.clone();
                self.token = self.get_token();
            } else {
                self.param.clear();
            }
        } else {
            self.param.clear();
            self.token.clear();
        }
        String::from_utf8_lossy(&self.param).into_owned()
    }

    /// Pascal `TParser.MakeString` (`ParserDel.pas:536-541`).
    pub fn str_value(&self) -> String {
        String::from_utf8_lossy(&self.token).into_owned()
    }

    /// Pascal `TParser.MakeDouble` (`ParserDel.pas:578-596`): empty token →
    /// 0.0; quoted token → RPN (reduced: plain number only, else trap);
    /// otherwise `Val` (Rust f64 parse — both correctly rounded).
    pub fn dbl_value(&self) -> f64 {
        if self.token.is_empty() {
            return 0.0;
        }
        let s = String::from_utf8_lossy(&self.token);
        // Both branches accept exactly a plain number here; see module doc
        // for the deliberate RPN reduction on the quoted path.
        match s.trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => {
                if self.is_quoted {
                    panic!(
                        "indmach012a: RPN inline math not supported by the wasm fixture: \"{s}\""
                    );
                }
                panic!("indmach012a: floating point conversion error for string: \"{s}\"");
            }
        }
    }
}

/// Pascal `MainUnit.PropertyName[1..9]` (`MainUnit.pas:204-221`), stored
/// lowercase (THashList.Add lowercases, `HashList.pas:268`).
const PROPERTY_NAMES: [&str; 9] = [
    "rs", "xs", "rr", "xr", "xm", "slip", "maxslip", "option", "help",
];

/// Pascal `TCommandList.GetCommand` (`Command.pas:80-90`) with
/// `Abbrev = TRUE`: case-insensitive exact match (`THashList.Find`), else
/// first prefix match in insertion order (`THashList.FindAbbrev`,
/// `HashList.pas:335-357`). Returns the 1-based property index or 0.
pub fn get_command(cmd: &str) -> i32 {
    let t = cmd.to_ascii_lowercase();
    for (i, name) in PROPERTY_NAMES.iter().enumerate() {
        if *name == t {
            return (i + 1) as i32;
        }
    }
    if !t.is_empty() {
        for (i, name) in PROPERTY_NAMES.iter().enumerate() {
            if name.starts_with(&t) {
                return (i + 1) as i32;
            }
        }
    }
    0
}
