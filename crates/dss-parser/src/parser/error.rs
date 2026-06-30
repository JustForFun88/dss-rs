//! The parser's conversion/inline-math failure type (Pascal `EParserProblem`).

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
