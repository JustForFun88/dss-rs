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

        // Solution guard (`ExportOptions.pas:163-177`): the solution-reading
        // exports need a solved circuit. The **no-circuit** half (#24711) is
        // unreachable from here — `ProcessCommand`'s generic pre-circuit guard
        // (#301, `command.rs`) already fires before this router when no circuit
        // exists (`ExecCommands.pas:364`; Export is not in the OK-before-circuit
        // list). So only the "must be solved" (#24712) check can fire: it does
        // when the circuit exists but `Solution.NodeV` is unallocated (the Vec is
        // `[ground]` only — no solve/Y-build yet).
        if matches!(ptr, 1..=24 | 28..=32 | 35 | 46..=51)
            && self
                .circuit
                .as_ref()
                .is_some_and(|c| c.solution.node_v.len() <= 1)
        {
            self.errors
                .push("The circuit must be solved before you can do this.".to_string());
            return;
        }

        if ptr == 0 {
            // Pascal error 24713 (`'Error: Unknown Export command: "%s"'`).
            self.errors
                .push(format!("Error: Unknown Export command: \"{parm1}\""));
            return;
        }

        // The optional trailing filename (Pascal reads it after any per-report
        // pre-parsing; none of the WP8.2 step-1 exports pre-parse a `Parm2`).
        // TODO(WP8.2): reports 8/9/15/17/19/20-21/32/51 consume a `Parm2` (kVA/MVA
        // flag, monitor name, meter, …) BEFORE the filename (`ExportOptions.pas:190`).
        // When they land, move their per-report pre-parsing ahead of this read so
        // their `Parm2` is not mis-consumed as the filename.
        self.parser.next_param(&self.vars);
        let explicit = self.parser.make_string(&self.vars);

        use crate::report::export;
        match ptr {
            1 => self.export_with(&explicit, "EXP_VOLTAGES.csv", export::export_voltages),
            23 => self.export_with(&explicit, "EXP_BUSCOORDS.csv", export::export_bus_coords),
            26 => self.export_counts_to_file(&explicit), // Counts (WP8.1)
            39 => self.export_with(&explicit, "EXP_NodeNames.csv", export::export_node_names),
            46 => self.export_with(&explicit, "EXP_YNodeList.csv", export::export_ynode_list),
            _ => {
                let name = EXPORT_OPTIONS[ptr - 1];
                self.errors
                    .push(format!("Export \"{name}\" is not ported yet (Phase 8)."));
            }
        }
    }

    /// Run a read-only circuit formatter `f` and write its output to the report
    /// file (the shared path for the solution exports). The circuit is always
    /// present: `ProcessCommand`'s generic pre-circuit guard (#301, `command.rs`)
    /// dispatches `Export` only after a circuit exists — for *every* keyword,
    /// including the ones that skip the solution guard (e.g. NodeNames, ptr 39).
    fn export_with(&mut self, explicit: &str, default_name: &str, f: fn(&Circuit) -> String) {
        let content = f(self.circuit.as_ref().expect("post-circuit dispatch"));
        self.write_export(explicit, default_name, &content);
    }

    /// `Export Counts` (Pascal `ExportCounts`): dump every class + its instance
    /// count to `<OutputDirectory><CircuitName_>EXP_Counts.csv` (or the explicit
    /// filename). No solve dependency — the count is over the live registry.
    fn export_counts_to_file(&mut self, explicit: &str) {
        let class_counts: Vec<(String, usize)> = self
            .classes
            .iter()
            .map(|c| (c.props.class_name().to_string(), c.objects.len()))
            .collect();
        let content = crate::report::export::export_counts(&class_counts);
        self.write_export(explicit, "EXP_Counts.csv", &content);
    }

    /// Resolve the report path, write `content`, and record the export-specific
    /// `@lastexportfile` (Pascal `DoExportCmd`: `SetLastResultFile` +
    /// `ParserVars.Add('@lastexportfile', …)`, `ExportOptions.pas:635-636`). The
    /// default filename is prefixed with `<OutputDirectory><CircuitName_>`, where
    /// `CircuitName_ = <CaseName>_` and `CaseName` defaults to the circuit name
    /// (`Circuit.pas` `Set_CaseName`); an explicit `Export <x> <file>` argument
    /// overrides it verbatim. Show/Save do NOT set `@lastexportfile` (Show sets
    /// neither; Save sets `@lastfile` + `GlobalResult`), so it lives here, not in
    /// the generic `write_report`.
    fn write_export(&mut self, explicit: &str, default_name: &str, content: &str) {
        let case = self
            .circuit
            .as_ref()
            .map(|c| c.case_name.clone())
            .unwrap_or_default();
        let circuit_name_ = format!("{case}_");
        let path = crate::report::output::export_path(
            &self.output_directory,
            &circuit_name_,
            explicit,
            default_name,
        );
        if self.write_report(&path, content) {
            let p = self.last_result_file.clone();
            self.vars.add("@lastexportfile", &p);
        }
    }

    /// Write a finished report to `path` and record it as the last result file
    /// (Pascal `SetLastResultFile`: `@lastfile` + `LastResultFile`). Returns
    /// whether the write succeeded (a failure is recorded, not swallowed). The
    /// generic writer shared by `Export`/`Show`/`Save`; the per-verb extras
    /// (`@lastexportfile` / `GlobalResult`) are set by the caller.
    fn write_report(&mut self, path: &Path, content: &str) -> bool {
        match std::fs::write(path, content) {
            Ok(()) => {
                let p = path.to_string_lossy().into_owned();
                self.vars.add("@lastfile", &p);
                self.last_result_file = p;
                true
            }
            Err(e) => {
                self.errors.push(format!(
                    "Error writing report file \"{}\": {e}",
                    path.display()
                ));
                false
            }
        }
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
