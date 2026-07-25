//! Unified engine diagnostics (`DE_PASCALIZE` P5).
//!
//! One `miette`-based diagnostic type backs every error the engine records —
//! the `DoSimpleMsg`/`DoErrorMsg` log (Pascal `DSSGlobals.pas`), the deferred
//! property-hook channel, and the wrapped typed errors (`ParserError`,
//! `SparseError`, `SingularMatrix`). `miette` is used as a *protocol* only (no
//! `fancy` feature here): the library carries structured `Diagnostic` data —
//! `code`/`severity`/`help` (and, from P5b, source spans) — and the binary
//! (`dss-cli`) owns terminal presentation.
//!
//! The **stable identity** of an error is its numeric `code` (the Pascal
//! `DoSimpleMsg(..., NNN)` number, rendered `dss::eNNN`), never its message
//! text: post-acceptance the wording is free to change (plan §P5 golden
//! policy), so tests and the future GUI key on the code, not the string.

use std::fmt;
use std::ops::{Deref, DerefMut};

use miette::{Diagnostic, LabeledSpan, NamedSource, Severity, SourceCode, SourceSpan};

/// One engine diagnostic — the single error type for the whole crate.
///
/// Constructed via [`DssDiagnostic::msg`] (`DoSimpleMsg` — record and continue)
/// or [`DssDiagnostic::abort`] (`DoErrorMsg` — also requests `SolutionAbort`).
/// `span`/`src` stay `None` until P5b attaches source spans; `help` is an
/// optional hint the message-cleanup pass may fill in.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{message}")]
pub struct DssDiagnostic {
    /// Free-form human text. **Not** a contract — see the module docs.
    pub message: String,
    /// The Pascal `DoSimpleMsg`/`DoErrorMsg` number, rendered `dss::eNNN`. The
    /// stable identity of the error; `None` when the Pascal source cites no
    /// number (never invented).
    pub code: Option<u32>,
    /// `true` = `DoErrorMsg` semantics (severity Error + `SolutionAbort`);
    /// `false` = `DoSimpleMsg` (severity Warning, record-and-continue).
    pub abort: bool,
    /// Source span of the offending token (P5b; `None` until then).
    pub span: Option<SourceSpan>,
    /// The command line / file the span points into (P5b; `None` until then).
    pub src: Option<NamedSource<String>>,
    /// Optional "valid range is …"-style hint.
    pub help: Option<String>,
}

impl DssDiagnostic {
    /// `DoSimpleMsg` — record-and-continue. `code` is the Pascal number the
    /// adjacent source cites, or `None`.
    pub fn msg(message: impl Into<String>, code: Option<u32>) -> Self {
        Self {
            message: message.into(),
            code,
            abort: false,
            span: None,
            src: None,
            help: None,
        }
    }

    /// `DoErrorMsg` — record and request a solution abort.
    pub fn abort(message: impl Into<String>, code: Option<u32>) -> Self {
        Self {
            abort: true,
            ..Self::msg(message, code)
        }
    }

    /// Builder: set the numeric code.
    pub fn with_code(mut self, code: u32) -> Self {
        self.code = Some(code);
        self
    }

    /// Builder: attach a source span + its source (P5b).
    pub fn with_span(mut self, span: impl Into<SourceSpan>, src: NamedSource<String>) -> Self {
        self.span = Some(span.into());
        self.src = Some(src);
        self
    }

    /// Builder: attach a help hint.
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// The message text (Pascal logged just this string).
    pub fn text(&self) -> &str {
        &self.message
    }
}

impl Diagnostic for DssDiagnostic {
    fn code<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.code
            .map(|n| Box::new(format!("dss::e{n}")) as Box<dyn fmt::Display>)
    }

    fn severity(&self) -> Option<Severity> {
        Some(if self.abort {
            Severity::Error
        } else {
            Severity::Warning
        })
    }

    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.help
            .as_deref()
            .map(|h| Box::new(h) as Box<dyn fmt::Display>)
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        self.src.as_ref().map(|s| s as &dyn SourceCode)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = LabeledSpan> + '_>> {
        self.span.map(|span| {
            Box::new(std::iter::once(LabeledSpan::underline(span)))
                as Box<dyn Iterator<Item = LabeledSpan>>
        })
    }
}

/// A diagnostic views (read-only) as its message text, so `&DssDiagnostic`
/// coerces to `&str` for the ubiquitous `errors().iter().any(|e|
/// e.contains(...))` presence checks. Text is not the error's identity (the
/// `code` is) — this is a display convenience, not a contract.
impl Deref for DssDiagnostic {
    type Target = str;
    fn deref(&self) -> &str {
        &self.message
    }
}

impl From<String> for DssDiagnostic {
    fn from(message: String) -> Self {
        Self::msg(message, None)
    }
}

impl From<&str> for DssDiagnostic {
    fn from(message: &str) -> Self {
        Self::msg(message, None)
    }
}

impl From<&String> for DssDiagnostic {
    fn from(message: &String) -> Self {
        Self::msg(message.clone(), None)
    }
}

/// The parser's conversion/inline-math failure (Pascal `EParserProblem`) wrapped
/// at its catch sites — the message text is preserved, the code stays `None`.
/// P5b carries the offending token's byte range through `ParserError::span`; the
/// `src` is still `None` here (the executive attaches the command-line source
/// where it knows the origin), so a bare span never renders a source snippet on
/// its own — miette needs both `source_code()` and `labels()`.
impl From<dss_parser::ParserError> for DssDiagnostic {
    fn from(e: dss_parser::ParserError) -> Self {
        let span = e.span();
        let mut d = Self::msg(e.message().to_string(), None);
        d.span = span.map(|r| (r.start, r.len()).into());
        d
    }
}

/// The pure-Rust sparse solver's failure (`dss-sparse`, Pascal KLUSolve return
/// codes) wrapped where caught in the solve/auto-add/diakoptics paths.
impl From<dss_sparse::SparseError> for DssDiagnostic {
    fn from(e: dss_sparse::SparseError) -> Self {
        Self::msg(e.to_string(), None)
    }
}

/// A singular dense-matrix inversion (`TcMatrix.Invert` `InvertError = 2`).
impl From<crate::support::cmatrix::SingularMatrix> for DssDiagnostic {
    fn from(e: crate::support::cmatrix::SingularMatrix) -> Self {
        Self::msg(e.to_string(), None)
    }
}

/// The engine's record-and-continue error log — a typed `Vec<DssDiagnostic>`
/// with a `String`-accepting [`ErrorLog::push`], so the ~220 `errors.push(...)`
/// sites that carry no Pascal number keep pushing plain text (→ `code: None`,
/// exactly the policy for uncited sites) while numbered sites push an explicit
/// [`DssDiagnostic::msg`]. Derefs to the inner `Vec` for every read/iter.
#[derive(Debug, Default, Clone)]
pub struct ErrorLog(Vec<DssDiagnostic>);

impl ErrorLog {
    pub fn new() -> Self {
        Self(Vec::new())
    }

    /// Record a diagnostic. Accepts a bare `String`/`&str` (→ `code: None`) or a
    /// fully-built [`DssDiagnostic`]. Shadows the `Vec::push` reachable through
    /// `DerefMut` (inherent methods win) so every call site stays typed.
    pub fn push(&mut self, d: impl Into<DssDiagnostic>) {
        self.0.push(d.into());
    }

    /// The message texts only — the convenience the pre-P5 harness callers used
    /// (`Dss::error_texts`).
    pub fn texts(&self) -> Vec<String> {
        self.0.iter().map(|d| d.message.clone()).collect()
    }

    pub fn into_vec(self) -> Vec<DssDiagnostic> {
        self.0
    }
}

impl Deref for ErrorLog {
    type Target = Vec<DssDiagnostic>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for ErrorLog {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<Vec<DssDiagnostic>> for ErrorLog {
    fn from(v: Vec<DssDiagnostic>) -> Self {
        Self(v)
    }
}

impl Extend<DssDiagnostic> for ErrorLog {
    fn extend<T: IntoIterator<Item = DssDiagnostic>>(&mut self, iter: T) {
        self.0.extend(iter);
    }
}

/// Convenience so a legacy `Vec<String>` channel can be drained straight into
/// the typed log (each string → `code: None`). Mirrors `String`'s own many
/// `Extend<T>` impls; the item type disambiguates the call.
impl Extend<String> for ErrorLog {
    fn extend<T: IntoIterator<Item = String>>(&mut self, iter: T) {
        self.0.extend(iter.into_iter().map(DssDiagnostic::from));
    }
}

impl FromIterator<DssDiagnostic> for ErrorLog {
    fn from_iter<T: IntoIterator<Item = DssDiagnostic>>(iter: T) -> Self {
        Self(iter.into_iter().collect())
    }
}

impl IntoIterator for ErrorLog {
    type Item = DssDiagnostic;
    type IntoIter = std::vec::IntoIter<DssDiagnostic>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a ErrorLog {
    type Item = &'a DssDiagnostic;
    type IntoIter = std::slice::Iter<'a, DssDiagnostic>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_is_the_message_verbatim() {
        let d = DssDiagnostic::msg("Object Class \"widget\" not found", Some(705));
        assert_eq!(d.to_string(), "Object Class \"widget\" not found");
        assert_eq!(d.text(), "Object Class \"widget\" not found");
    }

    #[test]
    fn code_renders_dss_e_number() {
        let d = DssDiagnostic::msg("boom", Some(705));
        assert_eq!(d.code().unwrap().to_string(), "dss::e705");
        let none = DssDiagnostic::msg("boom", None);
        assert!(none.code().is_none());
    }

    #[test]
    fn severity_flips_on_abort() {
        assert_eq!(
            DssDiagnostic::msg("x", None).severity(),
            Some(Severity::Warning)
        );
        assert_eq!(
            DssDiagnostic::abort("x", None).severity(),
            Some(Severity::Error)
        );
        assert!(!DssDiagnostic::msg("x", None).abort);
        assert!(DssDiagnostic::abort("x", None).abort);
    }

    #[test]
    fn help_delegates_to_field() {
        let d = DssDiagnostic::msg("x", None).with_help("valid range is 1..=3");
        assert_eq!(d.help().unwrap().to_string(), "valid range is 1..=3");
        assert!(DssDiagnostic::msg("x", None).help().is_none());
    }

    #[test]
    fn string_converts_to_uncoded_diagnostic() {
        let d: DssDiagnostic = String::from("bare").into();
        assert_eq!(d.message, "bare");
        assert_eq!(d.code, None);
        assert!(!d.abort);
    }

    #[test]
    fn error_log_push_accepts_string_and_diagnostic() {
        let mut log = ErrorLog::new();
        log.push(String::from("plain"));
        log.push("slice");
        log.push(DssDiagnostic::msg("coded", Some(42)));
        assert_eq!(log.len(), 3);
        assert_eq!(log[0].code, None);
        assert_eq!(log[2].code, Some(42));
        assert_eq!(log.texts(), vec!["plain", "slice", "coded"]);
    }
}
