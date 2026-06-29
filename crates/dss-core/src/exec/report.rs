//! The `Export`/`Show`/`Save`/`Dump` command routers (Pascal
//! `Executive/ExportOptions.pas` `DoExportCmd`, `Executive/ShowOptions.pas`
//! `DoShowCmd`, `Executive/ExecHelper.pas` `DoSaveCmd`/`DumpProperties`).
//!
//! These are `impl Dss` like every other command router (`command.rs`,
//! `set_cmd.rs`): they drive the private parser/circuit/error state and
//! delegate all formatting to the top-level [`crate::report`] module. This is
//! the WP8.1 dispatch **skeleton** — keyword matching + scoped `NOT_PORTED`
//! stubs; the real per-report formatters land in WP8.2–8.5.

use super::*;
use crate::report::EXPORT_OPTIONS;

impl Dss {
    /// Pascal `DoExportCmd` (`ExportOptions.pas:127`): read the report keyword,
    /// resolve it against `ExportCommands`, dispatch to the matching exporter.
    ///
    /// WP8.1 skeleton: every keyword records a **scoped `NOT_PORTED`** until its
    /// WP lands (WP8.2 solution exports, WP8.3 device/meter/reliability, Phase 9
    /// CIM/GIC/ADiakoptics — PHASE8_PLAN §WP8.1 step 1 / §4) so a half-ported
    /// `Export` never silently emits nothing. All `Export` corpus decks sit in
    /// `skipped_unsupported`, so these errors never reach the live gate.
    pub(crate) fn do_export_cmd(&mut self) {
        // Pascal `ParamName := DSS.Parser.NextParam; Parm1 := AnsiLowerCase(...)`.
        self.parser.next_param(&self.vars);
        let parm1 = self.parser.make_string(&self.vars).to_lowercase();
        let ptr = self
            .export_commands
            .get_command(&parm1)
            .map(|i| i + 1)
            .unwrap_or(0);
        if ptr == 0 {
            // Pascal error 24713 (`'Error: Unknown Export command: "%s"'`).
            self.errors
                .push(format!("Error: Unknown Export command: \"{parm1}\""));
            return;
        }
        let name = EXPORT_OPTIONS[ptr - 1];
        self.errors
            .push(format!("Export \"{name}\" is not ported yet (Phase 8)."));
    }

    /// Pascal `DoShowCmd` (`ShowOptions.pas:89`): read the report keyword and
    /// dispatch to the matching `ShowResults` formatter.
    ///
    /// WP8.1 skeleton: every `Show` report is a **faithful headless no-op** — it
    /// writes a report file but changes **no** electrical state (PHASE8_PLAN
    /// §2.5; the live gate compares the assembled model, not report text). WP8.4
    /// replaces each keyword with the real formatter + a targeted text golden,
    /// and adds the oracle's `Show panel`→999 / unknown→24700 errors. The no-op
    /// stays *silent* (no recorded error) until then so it cannot regress the 44
    /// `solvable_now` decks that contain `Show Power`/`Show Voltage`/`Show f`
    /// (the live gate asserts `errors().is_empty()`).
    pub(crate) fn do_show_cmd(&mut self) {
        // Pascal `DSS.Parser.NextParam; Param := AnsiLowerCase(StrValue)`.
        self.parser.next_param(&self.vars);
        let param = self.parser.make_string(&self.vars).to_lowercase();
        // Resolve against ShowCommands so the dispatch table is wired and ready
        // for WP8.4; the result is intentionally unused until the formatters land.
        let _ptr = self.show_commands.get_command(&param);
    }

    /// Pascal `DoSaveCmd` (`ExecHelper.pas`): `Save circuit` / `Save <class>` /
    /// `Save meters`/`Save voltages`. WP8.5 — scoped `NOT_PORTED` until then
    /// (the `Save` corpus decks are in `skipped_unsupported`).
    pub(crate) fn do_save_cmd(&mut self) {
        self.errors
            .push("Save is not ported yet (Phase 8 WP8.5).".to_string());
    }

    /// Pascal `DumpProperties` (the `Dump` command, `ExecHelper.pas`). WP8.5 —
    /// scoped `NOT_PORTED` until then (the `Dump` corpus decks are in
    /// `skipped_unsupported`).
    pub(crate) fn do_dump_cmd(&mut self) {
        self.errors
            .push("Dump is not ported yet (Phase 8 WP8.5).".to_string());
    }
}
