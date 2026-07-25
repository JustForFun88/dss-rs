//! The low-level byte scanner: the whitespace/delimiter/comment predicates and
//! the `GetToken` core (Pascal `SkipWhiteSpace`/`IsDelimiter`/`GetToken`).

use super::Parser;

const COMMENT_CHAR: u8 = b'!';

/// True when `b` occurs in `set`, ASCII-only (a multi-byte char in `set` can
/// never match, just as in the byte-based Pascal scanner).
fn in_set(set: &str, b: u8) -> bool {
    b.is_ascii() && set.as_bytes().contains(&b)
}

impl Parser {
    fn is_white_space(&self, b: u8) -> bool {
        in_set(&self.white_space_chars, b)
    }

    fn is_delim_char(&self, b: u8) -> bool {
        in_set(&self.delim_chars, b)
    }

    /// Pascal `SkipWhiteSpace`: never examines the last character of the
    /// buffer (the appended space).
    pub(super) fn skip_white_space(&self, buf: &str, pos: &mut usize) {
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
    pub(super) fn get_token_at(&mut self, buf: &str, pos: &mut usize) -> String {
        let bytes = buf.as_bytes();
        let len = bytes.len();
        let mut result = String::new();
        // P5b: default to a zero-width span at the cursor (an empty token at
        // end-of-line); overwritten below when a token is actually scanned.
        self.tok_start = *pos;
        self.tok_end = *pos;

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
                // P5b: the span covers the quoted content (between the quotes),
                // recorded before we step past the closing quote.
                self.tok_start = start;
                self.tok_end = *pos;
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
                // P5b: span of the bare token content.
                self.tok_start = start;
                self.tok_end = *pos;
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
}
