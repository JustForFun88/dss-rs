//! The parser's conversion/inline-math failure type (Pascal `EParserProblem`).

use std::ops::Range;

/// Conversion or inline-math failure (Pascal `EParserProblem`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParserError {
    message: String,
    /// Byte range of the offending token in the parser's `cmd_string`, when the
    /// raiser knew it (P5b). `None` for errors raised without a live token
    /// context. Consumed by `dss-core`'s `From<ParserError>` to underline the
    /// token in the rendered diagnostic.
    span: Option<Range<usize>>,
    /// The diagnostic number, when the raiser gives one. Copied by `dss-core`'s
    /// `From<ParserError>` into the logged diagnostic.
    code: Option<u32>,
}

impl ParserError {
    /// Build an error carrying a message. Public because the engine crate
    /// reuses this type wherever the Pascal original raised an exception that
    /// `ProcessCommand` would catch (`EParserProblem` and plain `Exception`).
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
            code: None,
        }
    }

    /// Attach the byte range of the offending token (P5b builder).
    pub fn with_span(mut self, span: Range<usize>) -> Self {
        self.span = Some(span);
        self
    }

    /// Attach the diagnostic number.
    pub fn with_code(mut self, code: u32) -> Self {
        self.code = Some(code);
        self
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    /// Byte range of the offending token in the parser source, if known (P5b).
    pub fn span(&self) -> Option<Range<usize>> {
        self.span.clone()
    }

    /// The diagnostic number, if the raiser gave one.
    pub fn code(&self) -> Option<u32> {
        self.code
    }
}

impl std::fmt::Display for ParserError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParserError {}
