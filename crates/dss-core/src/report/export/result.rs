//! `Export Result` (Pascal `ExportResults.pas` `ExportResult`): dump the value
//! of the `@result` parser variable, one line.
//!
//! Pascal `ExportResult` does `ParserVars.Lookup('@result'); FSWriteln(F, Value)`.
//! In the **pinned oracle** (dss-python 0.15.7, a `DSS_CAPI_PM` build) the
//! `@result := GlobalResult` update at the tail of `ProcessCommand`
//! (`ExecCommands.pas:704`) is compiled out (`{$IFNDEF DSS_CAPI_PM}`), so
//! `@result` stays at its intrinsic init value `'null'` (`ParserDel.pas:875`)
//! forever. Our engine likewise never writes `@result` (`ParserVars::new` seeds
//! it to `"null"`), so this reproduces the oracle's observable output — a single
//! `null` line — with no divergence.

/// Build the `Export Result` body: the `@result` value followed by a newline
/// (Pascal `FSWriteln`). `value` is the caller's `vars.get("@result")`.
pub fn export_result(value: &str) -> String {
    format!("{value}\n")
}
