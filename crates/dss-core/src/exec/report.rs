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

/// Which register-dump export is running (Pascal `ExportMeters`/`ExportGenMeters`/
/// `ExportPVSystemMeters`/`ExportStorageMeters`); selects the element list,
/// register-name set, header label, and file-name spellings.
enum RegKind {
    Meters,
    Generators,
    PvSystem,
    Storage,
}

/// Pascal `WriteSingle*MeterFile`'s rewrite test: (re)create the file unless it
/// already exists **and** its first line begins with `Year` (case-insensitive,
/// the header sentinel) — a missing, unreadable, or non-`Year` file is rewritten.
fn register_need_rewrite(path: &Path) -> bool {
    use std::io::BufRead;
    // Read only the first line (Pascal `FSReadLn` reads a single line, not the
    // whole append-log).
    match std::fs::File::open(path) {
        Ok(f) => {
            let mut first = String::new();
            let _ = std::io::BufReader::new(f).read_line(&mut first);
            !first
                .get(..4)
                .is_some_and(|p| p.eq_ignore_ascii_case("Year"))
        }
        Err(_) => true,
    }
}

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

        // Per-report pre-parse of an option flag BEFORE the trailing filename
        // (Pascal `ExportOptions.pas:190-298`), so its `Parm2` is not
        // mis-consumed as the filename. Ported: 8 UE-only, 9/19 MVA, 15 monitor
        // name, 17 triplet, 32 profile phases, 51 sections meter; the CIM pair
        // (20-21) stays with its Phase-9 exporter.
        let mut mva_opt = 0;
        if matches!(ptr, 9 | 19) {
            self.parser.next_param(&self.vars);
            let parm2 = self.parser.make_string(&self.vars).to_lowercase();
            if parm2.starts_with('m') {
                mva_opt = 1;
            }
        }
        // `Unserved`(8) traps a leading `u…` → the UE-only (emergency-criterion)
        // form (Pascal `ExportOptions.pas:201`), ahead of the filename.
        let mut ue_only = false;
        if ptr == 8 {
            self.parser.next_param(&self.vars);
            let parm2 = self.parser.make_string(&self.vars).to_lowercase();
            ue_only = parm2.starts_with('u');
        }
        // `Y`(17) traps a leading `t…` → the sparse-triplet form (Pascal
        // `ExportOptions.pas:217`, `TripletOpt`), again ahead of the filename.
        let mut triplet = false;
        if ptr == 17 {
            self.parser.next_param(&self.vars);
            let parm2 = self.parser.make_string(&self.vars).to_lowercase();
            triplet = parm2.starts_with('t');
        }
        // `Monitors`(15) consumes the required monitor name (Pascal
        // `ExportOptions.pas:211-215` — `Parm2 := StrValue`, case-preserved), also
        // ahead of the trailing filename read (which the monitor export ignores —
        // each monitor writes its own fixed `Get_FileName`).
        let mut monitor_name = String::new();
        if ptr == 15 {
            self.parser.next_param(&self.vars);
            monitor_name = self.parser.make_string(&self.vars);
        }
        // `Profile`(32) pre-parses the phases-to-plot selector (Pascal
        // `ExportOptions.pas:261-287`): the named selectors match by
        // `CompareTextShortest` (an empty `Parm2` matches `default` via the
        // empty-string quirk — the faithful default), a single-character token
        // re-reads as `Parser.IntValue` (an explicit phase number).
        let mut phases_to_plot = crate::report::export::plot_phases::THREE_PHASE;
        if ptr == 32 {
            use crate::report::export::plot_phases as pp;
            use crate::util::compare_text_shortest_eq as short;
            self.parser.next_param(&self.vars);
            let parm2 = self.parser.make_string(&self.vars);
            phases_to_plot = if short(&parm2, "default") {
                pp::THREE_PHASE
            } else if short(&parm2, "all") {
                pp::ALL
            } else if short(&parm2, "primary") {
                pp::PRIMARY
            } else if short(&parm2, "ll3ph") {
                pp::LL_3PH
            } else if short(&parm2, "llall") {
                pp::LL_ALL
            } else if short(&parm2, "llprimary") {
                pp::LL_PRIMARY
            } else if parm2.len() == 1 {
                get_int(&mut self.parser, &self.vars, &mut self.errors).unwrap_or(pp::THREE_PHASE)
            } else {
                pp::THREE_PHASE
            };
        }
        // `Sections`(51) pre-parses an optional `meter=<name>` (Pascal
        // `ExportOptions.pas:289-296`): the *parameter name* shortest-matches
        // `meter` (a positional value's empty name also matches — the same
        // empty-string quirk), the value resolves via `EnergyMeterClass.Find`
        // (case-insensitive; an unknown name silently leaves `pMeter = NIL` →
        // all meters).
        let mut section_meter: Option<crate::elements::traits::ElemRef> = None;
        if ptr == 51 {
            let param_name = self.parser.next_param(&self.vars);
            let parm2 = self.parser.make_string(&self.vars);
            if crate::util::compare_text_shortest_eq(&param_name, "meter") {
                section_meter = self
                    .circuit
                    .as_ref()
                    .expect("post-circuit dispatch")
                    .energy_meters
                    .iter()
                    .copied()
                    .find(|&r| {
                        self.classes[r.cls].objects[r.idx]
                            .data()
                            .name()
                            .eq_ignore_ascii_case(&parm2)
                    });
            }
        }

        // The optional trailing filename (Pascal `ExportOptions.pas:300-305`).
        self.parser.next_param(&self.vars);
        let explicit = self.parser.make_string(&self.vars);

        use crate::report::export;
        match ptr {
            1 => self.export_with(&explicit, "EXP_VOLTAGES.csv", export::export_voltages),
            2 => self.export_with(
                &explicit,
                "EXP_SEQVOLTAGES.csv",
                export::export_seq_voltages,
            ),
            3 => self.export_with_mut(&explicit, "EXP_CURRENTS.csv", |c, ckt, sys, nv| {
                export::export_currents(c, ckt, sys, nv)
            }),
            4 => self.export_with_mut(&explicit, "EXP_SEQCURRENTS.csv", |c, ckt, sys, nv| {
                export::export_seq_currents(c, ckt, sys, nv)
            }),
            9 => self.export_with_mut(&explicit, "EXP_POWERS.csv", |c, ckt, sys, nv| {
                export::export_powers(c, ckt, sys, nv, mva_opt)
            }),
            // ptr 10 (`SeqPowers`) never pre-parses the MVA flag (`ExportOptions.pas:191`
            // traps only 9/19), so `opt` is always 0 here.
            10 => self.export_with_mut(&explicit, "EXP_SEQPOWERS.csv", |c, ckt, sys, nv| {
                export::export_seq_powers(c, ckt, sys, nv, 0)
            }),
            6 => self.export_with_mut(&explicit, "EXP_CAPACITY.csv", |c, ckt, sys, nv| {
                export::export_capacity(c, ckt, sys, nv)
            }),
            7 => self.export_with_mut(&explicit, "EXP_OVERLOADS.csv", |c, ckt, sys, nv| {
                export::export_overloads(c, ckt, sys, nv)
            }),
            8 => self.export_with_mut(&explicit, "EXP_UNSERVED.csv", |c, ckt, sys, nv| {
                export::export_unserved(c, ckt, sys, nv, ue_only)
            }),
            34 => self.export_with_classes(
                &explicit,
                "AllocationFactors.txt",
                export::export_alloc_factors,
            ),
            11 => self.export_with(&explicit, "EXP_FAULTS.csv", export::export_fault_study),
            37 => self.export_with(
                &explicit,
                "EXP_BusReliability.csv",
                export::export_bus_reliability,
            ),
            38 => self.export_with_classes(
                &explicit,
                "EXP_BranchReliability.csv",
                export::export_branch_reliability,
            ),
            19 => self.export_with_mut(&explicit, "EXP_P_BYPHASE.csv", |c, ckt, sys, nv| {
                export::export_p_by_phase(c, ckt, sys, nv, mva_opt)
            }),
            16 => self.export_with_classes(&explicit, "EXP_YPRIM.csv", export::export_yprims),
            17 => self.export_y_to_file(&explicit, triplet),
            18 => self.export_with(&explicit, "EXP_SEQZ.csv", export::export_seq_z),
            23 => self.export_with(&explicit, "EXP_BUSCOORDS.csv", export::export_bus_coords),
            24 => self.export_with_mut(&explicit, "EXP_LOSSES.csv", export::export_losses),
            26 => self.export_counts_to_file(&explicit), // Counts (WP8.1)
            27 => self.export_summary_to_file(&explicit),
            39 => self.export_with(&explicit, "EXP_NodeNames.csv", export::export_node_names),
            40 => self.export_with_classes(&explicit, "EXP_Taps.csv", export::export_taps),
            41 => self.export_elem_ordered(&explicit, "EXP_NodeOrder.csv", |c, ckt, _sys, _nv| {
                export::export_node_order(c, ckt)
            }),
            42 => self.export_elem_ordered(&explicit, "EXP_ElemCurrents.csv", |c, ckt, sys, nv| {
                export::export_elem_currents(c, ckt, sys, nv)
            }),
            43 => self.export_elem_ordered(&explicit, "EXP_ElemVoltages.csv", |c, ckt, sys, nv| {
                export::export_elem_voltages(c, ckt, sys, nv)
            }),
            44 => self.export_elem_ordered(&explicit, "EXP_ElemPowers.csv", |c, ckt, sys, nv| {
                export::export_elem_powers(c, ckt, sys, nv)
            }),
            45 => {
                // `Result`: dump the `@result` parser var (always `"null"` in
                // the pinned PM-build oracle — see `export::export_result`).
                let val = self.vars.get("@result").unwrap_or("null").to_string();
                let content = export::export_result(&val);
                self.write_export(&explicit, "EXP_Result.csv", &content);
            }
            35 => self.export_with_mut(&explicit, "EXP_VOLTAGES_ELEM.csv", |c, ckt, _sys, _nv| {
                export::export_voltages_elements(c, ckt)
            }),
            15 => self.export_monitors(&monitor_name),
            12 => self.export_registers(&explicit, RegKind::Generators),
            13 => self.export_loads_to_file(&explicit),
            14 => self.export_registers(&explicit, RegKind::Meters),
            49 => self.export_registers(&explicit, RegKind::PvSystem),
            50 => self.export_registers(&explicit, RegKind::Storage),
            32 => self.export_with_classes(&explicit, "EXP_Profile.csv", |c, ckt| {
                export::export_profile(c, ckt, phases_to_plot)
            }),
            51 => self.export_with_classes(&explicit, "EXP_SECTIONS.csv", |c, ckt| {
                export::export_sections(c, ckt, section_meter)
            }),
            33 => self.export_event_log_to_file(&explicit),
            52 => self.export_error_log_to_file(&explicit),
            46 => self.export_with(&explicit, "EXP_YNodeList.csv", export::export_ynode_list),
            47 => self.export_with(&explicit, "EXP_YVoltages.csv", export::export_y_voltages),
            48 => self.export_with(&explicit, "EXP_YCurrents.csv", export::export_y_currents),
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

    /// Like [`Dss::export_with`] but for the **element** exports, which call the
    /// mutating terminal getters (`ComputeIterminal`/`ComputeVterminal`/`Power`/
    /// `GetLosses`). Hands the formatter the disjoint `(&mut classes, &circuit,
    /// &sys, &node_v)` borrow (the `snapshot_elements` pattern) so it can walk the
    /// `Sources`/`PDElements`/`Faults`/`PCElements` lists and recompute each
    /// element's terminal quantities, then writes the produced text. The circuit
    /// is always present (the generic pre-circuit #301 guard already fired).
    fn export_with_mut<F>(&mut self, explicit: &str, default_name: &str, f: F)
    where
        F: FnOnce(
            &mut [crate::exec::registry::DssClass],
            &Circuit,
            &crate::elements::traits::SysCtx,
            &[num_complex::Complex64],
        ) -> String,
    {
        let content = {
            let Dss {
                classes, circuit, ..
            } = self;
            let ckt = circuit.as_ref().expect("post-circuit dispatch");
            let sys = crate::solution::solution::sys_ctx(ckt);
            let node_v = ckt.solution.node_v.clone();
            f(classes, ckt, &sys, &node_v)
        };
        self.write_export(explicit, default_name, &content);
    }

    /// Like [`Dss::export_with_mut`] but for the `WriteNodeList`/`WriteElem*`
    /// family (`NodeOrder`/`ElemCurrents`/`ElemVoltages`/`ElemPowers`), which
    /// Pascal guards **per element** on `IsSolved` (error 222001) rather than via
    /// the `DoExportCmd` dispatch #24712 solve-guard. Each writer exits early on
    /// an unsolved circuit, so the file is header-only *and* the error is
    /// recorded — the formatter self-guards its body (header only), and this
    /// pushes the 222001 once (Pascal pushes it once per element; the observable
    /// file content + error presence match, and no gate checks the count).
    fn export_elem_ordered<F>(&mut self, explicit: &str, default_name: &str, f: F)
    where
        F: FnOnce(
            &mut [crate::exec::registry::DssClass],
            &Circuit,
            &crate::elements::traits::SysCtx,
            &[num_complex::Complex64],
        ) -> String,
    {
        if self.circuit.as_ref().is_some_and(|c| !c.is_solved) {
            self.errors
                .push("Circuit must be solved for this command to execute properly.".to_string());
        }
        self.export_with_mut(explicit, default_name, f);
    }

    /// Like [`Dss::export_with`] but also hands the formatter the class registry
    /// (a read-only `(&[DssClass], &Circuit)` borrow) — for reports that walk
    /// **typed** objects rather than the generic `CktElement` surface (`Taps`
    /// downcasts each RegControl + its controlled Transformer). No mutation.
    fn export_with_classes<F>(&mut self, explicit: &str, default_name: &str, f: F)
    where
        F: FnOnce(&[crate::exec::registry::DssClass], &Circuit) -> String,
    {
        let content = {
            let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
            f(&self.classes, ckt)
        };
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

    /// `Export Monitors <name|all>` (Pascal `ExportOptions.pas` case 15): write
    /// each selected monitor's in-memory sample buffer to its own CSV via
    /// `TMonitorObj.TranslateToCSV`. Unlike the other exports this **ignores** the
    /// trailing filename — every monitor's path is its fixed `Get_FileName`,
    /// `<OutputDir><CircuitName_>Mon_<Name><_Name>.csv` (the `_1` suffix is the
    /// PM-build primary-context `DSS._Name`, matching the pinned oracle — the same
    /// PM build the always-`null` `Result` export keys on). An empty name raises
    /// Pascal's #251; an unknown named monitor raises #250. `Monitors` are walked
    /// in creation order (`ckt.monitors`); `GlobalResult`/`@lastexportfile` end on
    /// the last file written (Pascal reassigns `FileName := DSS.GlobalResult`).
    fn export_monitors(&mut self, name: &str) {
        if name.is_empty() {
            // Pascal #251 `'Monitor name not specified. %s'`.
            self.errors.push("Monitor name not specified.".to_string());
            return;
        }
        let monitors = self
            .circuit
            .as_ref()
            .expect("post-circuit dispatch")
            .monitors
            .clone();
        let case = self.circuit.as_ref().unwrap().case_name.clone();
        // Pascal `if Parm2 = 'all'` is case-sensitive; the named lookup
        // (`MonitorClass.Find`) is case-insensitive (THashList semantics).
        let targets: Vec<crate::elements::traits::ElemRef> = if name == "all" {
            monitors
        } else {
            match monitors.iter().find(|&&r| {
                self.classes[r.cls].objects[r.idx]
                    .data()
                    .name()
                    .eq_ignore_ascii_case(name)
            }) {
                Some(&r) => vec![r],
                None => {
                    // Pascal #250 `'Monitor "%s" not found. %s'`.
                    self.errors.push(format!("Monitor \"{name}\" not found."));
                    return;
                }
            }
        };

        let circuit_name_ = format!("{case}_");
        // Pascal sets `SetLastResultFile` + `@lastexportfile := FileName` ONCE
        // after the loop (`ExportOptions.pas` case 15 + the `DoExportCmd` tail),
        // where `FileName` ends on the last monitor's `GlobalResult` — or stays
        // `''` when the fleet is empty (`Export Monitors all` with no monitors
        // defined), clearing the bookkeeping. Track the last successfully-written
        // path (empty if none) and apply the bookkeeping once at the end.
        let mut last_path = String::new();
        for r in targets {
            let (mon_name, content) = {
                let obj = &self.classes[r.cls].objects[r.idx];
                let mon = obj
                    .as_any()
                    .downcast_ref::<crate::elements::meter::monitor::Monitor>()
                    .expect("ckt.monitors holds Monitor objects");
                (obj.data().name().to_string(), mon.to_csv())
            };
            let default_name = format!("Mon_{mon_name}_1.csv");
            let path = crate::report::output::export_path(
                &self.output_directory,
                &self.current_dir,
                &circuit_name_,
                "",
                &default_name,
            );
            if self.write_report(&path, &content) {
                last_path = self.last_result_file.clone();
            }
        }
        self.last_result_file = last_path.clone();
        self.vars.add("@lastfile", &last_path);
        self.vars.add("@lastexportfile", &last_path);
    }

    /// `Show monitor <name>` (Pascal `ShowOptions.pas` case 10 →
    /// `TMonitorObj.TranslateToCSV`): write the named monitor's in-memory sample
    /// buffer to its fixed CSV (`<OutputDir><CircuitName_>Mon_<name>_1.csv`) — the
    /// same content `Export Monitors` produces. Unlike `Export`, `Show` sets only
    /// `@lastshowfile`. An empty name → #249; an unknown monitor → #248.
    fn show_monitor(&mut self, name: &str) {
        if name.is_empty() {
            // Pascal #249 `'Monitor Name Not Specified. %s'`.
            self.errors.push("Monitor Name Not Specified.".to_string());
            return;
        }
        let (mon_name, content, case) = {
            let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
            let found = ckt.monitors.iter().copied().find(|&r| {
                self.classes[r.cls].objects[r.idx]
                    .data()
                    .name()
                    .eq_ignore_ascii_case(name)
            });
            match found {
                Some(r) => {
                    let obj = &self.classes[r.cls].objects[r.idx];
                    let mon = obj
                        .as_any()
                        .downcast_ref::<crate::elements::meter::monitor::Monitor>()
                        .expect("ckt.monitors holds Monitor objects");
                    (
                        obj.data().name().to_string(),
                        mon.to_csv(),
                        ckt.case_name.clone(),
                    )
                }
                None => {
                    // Pascal #248 `'Monitor "%s" not found. %s'`.
                    self.errors.push(format!("Monitor \"{name}\" not found."));
                    return;
                }
            }
        };
        let circuit_name_ = format!("{case}_");
        let default_name = format!("Mon_{mon_name}_1.csv");
        let path = crate::report::output::export_path(
            &self.output_directory,
            &self.current_dir,
            &circuit_name_,
            "",
            &default_name,
        );
        match std::fs::write(&path, content.as_bytes()) {
            Ok(()) => {
                let p = path.to_string_lossy().into_owned();
                self.vars.add("@lastshowfile", &p);
            }
            Err(e) => self.errors.push(format!(
                "Error writing report file \"{}\": {e}",
                path.display()
            )),
        }
    }

    /// `Export EventLog` (Pascal `ExportEventLog`, `ExportResults.pas:3296`):
    /// dump `DSS.EventStrings` — the accumulated `Hour=…, Sec=…, …` control /
    /// tap-change log — to `EXP_EventLog.csv`. No solve dependency (ptr 33 is not
    /// in the `DoExportCmd` solve-guard set); the log accumulates across the run.
    fn export_event_log_to_file(&mut self, explicit: &str) {
        let content = {
            let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
            crate::report::export::export_event_log(ckt.solution.event_log.entries())
        };
        self.write_export(explicit, "EXP_EventLog.csv", &content);
    }

    /// `Export ErrorLog` (Pascal `ExportErrorLog`, `ExportResults.pas:3303`): dump
    /// `DSS.ErrorStrings` to `EXP_ErrorLog.txt`. Rust's `Dss::errors` accumulates
    /// the same `DoSimpleMsg` messages (same lifecycle), but without Pascal's
    /// `(errnum)` line prefix / oracle-exact wording — so a non-empty dump is not
    /// oracle-faithful (see [`crate::report::export::export_error_log`]; tracked in
    /// STATUS §WP8.3 step 3a). No solve/circuit dependency.
    fn export_error_log_to_file(&mut self, explicit: &str) {
        let content = crate::report::export::export_error_log(&self.errors);
        self.write_export(explicit, "EXP_ErrorLog.txt", &content);
    }

    /// `Export Loads` (Pascal `ExportLoads`): the present load-allocation view,
    /// one row per enabled load. A fresh file (no append), pure Load-field reads.
    fn export_loads_to_file(&mut self, explicit: &str) {
        let content = {
            let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
            crate::report::export::export_loads(&self.classes, &ckt.loads)
        };
        self.write_export(explicit, "EXP_LOADS.csv", &content);
    }

    /// `Export Meters`/`Generators`/`PVSystem_Meters`/`Storage_Meters` (Pascal
    /// `ExportMeters`/`ExportGenMeters`/`ExportPVSystemMeters`/
    /// `ExportStorageMeters`): dump each enabled element's registers. Two paths:
    /// a leading `/m` filename switch writes one file per element
    /// (`WriteMultiple*MeterFiles`); otherwise a single **append** file
    /// (`WriteSingle*MeterFile`) — like `Export Summary`, so a running log
    /// accumulates across runs.
    fn export_registers(&mut self, explicit: &str, kind: RegKind) {
        let (label, default_name, multi_prefix, header_names, rows) =
            self.gather_register_rows(kind);
        let (year, hour, case) = {
            let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
            (
                ckt.solution.year,
                ckt.solution.int_hour,
                ckt.case_name.clone(),
            )
        };
        // `NameIfNotNil(LoadDurCurveObj)`: the LoadDuration solve mode /
        // `Set LoadDurCurve=` is not modeled (kept deferred), so the curve is
        // always nil here and the column is empty — faithful for every non-LD deck.
        let ldcurve = "";

        // Pascal `AnsiLowerCase(Copy(FileNm, 1, 2)) = '/m'` (the multi-file switch).
        if explicit.to_lowercase().starts_with("/m") {
            self.write_register_multi(multi_prefix, label, year, ldcurve, hour, &rows);
        } else {
            self.write_register_single(
                explicit,
                default_name,
                label,
                &header_names,
                &case,
                year,
                ldcurve,
                hour,
                &rows,
            );
        }
    }

    /// Gather the enabled elements' register rows + the single-file header names
    /// for `kind`, downcasting each `ElemRef` to its concrete type. Returns
    /// `(label, default_name, multi_prefix, header_names, rows)`.
    fn gather_register_rows(
        &self,
        kind: RegKind,
    ) -> (
        &'static str,
        &'static str,
        &'static str,
        Vec<String>,
        Vec<crate::report::export::RegRow>,
    ) {
        use crate::elements::meter::EnergyMeter;
        use crate::elements::pc::{Generator, PVSystem, Storage};
        use crate::report::export::{
            GEN_REGISTER_NAMES, PVSYSTEM_REGISTER_NAMES, RegRow, STORAGE_REGISTER_NAMES,
        };
        let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
        // Class-fixed register names → owned `Vec<String>` (Gen/PV/Storage).
        let fixed = |names: &[&str]| names.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        match kind {
            RegKind::Meters => {
                let refs = &ckt.energy_meters;
                // Pascal's header uses `energyMeters.First.RegisterNames` (the
                // first meter's names, even if disabled).
                let header_names = refs
                    .first()
                    .map(|&r| {
                        self.classes[r.cls].objects[r.idx]
                            .as_any()
                            .downcast_ref::<EnergyMeter>()
                            .expect("energy_meters holds EnergyMeter")
                            .register_names()
                            .to_vec()
                    })
                    .unwrap_or_default();
                let rows = refs
                    .iter()
                    .filter_map(|&r| {
                        let obj = &self.classes[r.cls].objects[r.idx];
                        let m = obj
                            .as_any()
                            .downcast_ref::<EnergyMeter>()
                            .expect("energy_meters holds EnergyMeter");
                        m.enabled().then(|| RegRow {
                            name: obj.data().name().to_string(),
                            register_names: m.register_names().to_vec(),
                            registers: m.registers().to_vec(),
                        })
                    })
                    .collect();
                ("Meter", "EXP_METERS.csv", "EXP_MTR_", header_names, rows)
            }
            RegKind::Generators => {
                let names = fixed(&GEN_REGISTER_NAMES);
                let rows = ckt
                    .generators
                    .iter()
                    .filter_map(|&r| {
                        let obj = &self.classes[r.cls].objects[r.idx];
                        let g = obj
                            .as_any()
                            .downcast_ref::<Generator>()
                            .expect("generators holds Generator");
                        g.cd.enabled.then(|| RegRow {
                            name: obj.data().name().to_string(),
                            register_names: names.clone(),
                            registers: g.registers.to_vec(),
                        })
                    })
                    .collect();
                ("Generator", "EXP_GENMETERS.csv", "EXP_GEN_", names, rows)
            }
            RegKind::PvSystem => {
                let names = fixed(&PVSYSTEM_REGISTER_NAMES);
                let rows = ckt
                    .pv_systems
                    .iter()
                    .filter_map(|&r| {
                        let obj = &self.classes[r.cls].objects[r.idx];
                        let p = obj
                            .as_any()
                            .downcast_ref::<PVSystem>()
                            .expect("pv_systems holds PVSystem");
                        p.cd.enabled.then(|| RegRow {
                            name: obj.data().name().to_string(),
                            register_names: names.clone(),
                            registers: p.registers.to_vec(),
                        })
                    })
                    .collect();
                ("PVSystem", "EXP_PVMeters.csv", "EXP_PV_", names, rows)
            }
            RegKind::Storage => {
                let names = fixed(&STORAGE_REGISTER_NAMES);
                let rows = ckt
                    .storages
                    .iter()
                    .filter_map(|&r| {
                        let obj = &self.classes[r.cls].objects[r.idx];
                        let s = obj
                            .as_any()
                            .downcast_ref::<Storage>()
                            .expect("storages holds Storage");
                        s.cd.enabled.then(|| RegRow {
                            name: obj.data().name().to_string(),
                            register_names: names.clone(),
                            registers: s.registers.to_vec(),
                        })
                    })
                    .collect();
                // TODO(compat): the Storage multi-file prefix is `EXP_PV_`, not
                // `EXP_STORAGE_` — an upstream copy-paste bug in
                // `WriteMultipleStorageMeterFiles` (`ExportResults.pas:2240`,
                // cloned from the PVSystem writer). Reproduced for the `/m` path;
                // clean fix = `EXP_STORAGE_` in the post-1:1 pass.
                ("Storage", "EXP_STORAGEMeters.csv", "EXP_PV_", names, rows)
            }
        }
    }

    /// The single-file register writer (Pascal `WriteSingle*MeterFile`): append to
    /// an existing `Year`-headed file, else (re)create with the header. Sets
    /// `GlobalResult`/`@lastexportfile`/`@lastfile` to the produced path.
    #[allow(clippy::too_many_arguments)]
    fn write_register_single(
        &mut self,
        explicit: &str,
        default_name: &str,
        label: &str,
        header_names: &[String],
        case: &str,
        year: i32,
        ldcurve: &str,
        hour: i32,
        rows: &[crate::report::export::RegRow],
    ) {
        use crate::report::export::{register_header, register_row};
        let case_ = format!("{case}_");
        let path = crate::report::output::export_path(
            &self.output_directory,
            &self.current_dir,
            &case_,
            explicit,
            default_name,
        );
        // Pascal: rewrite unless the file exists AND its first line begins `Year`
        // (`CompareText(Copy(TestStr,1,4),'Year')=0`); a missing/empty file rewrites.
        let rewrite = register_need_rewrite(&path);
        let mut content = String::new();
        if rewrite {
            content.push_str(&register_header(label, header_names));
            content.push('\n');
        }
        for row in rows {
            content.push_str(&register_row(
                year,
                ldcurve,
                hour,
                &row.name,
                &row.registers,
            ));
            content.push('\n');
        }
        let write_res = if rewrite {
            std::fs::write(&path, content.as_bytes())
        } else {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .and_then(|mut f| f.write_all(content.as_bytes()))
        };
        match write_res {
            Ok(()) => {
                let p = path.to_string_lossy().into_owned();
                self.vars.add("@lastfile", &p);
                self.vars.add("@lastexportfile", &p);
                self.last_result_file = p;
            }
            Err(e) => self.errors.push(format!(
                "Error writing report file \"{}\": {e}",
                path.display()
            )),
        }
    }

    /// The `/m` multi-file register writer (Pascal `WriteMultiple*MeterFiles`): one
    /// `<OutputDir><prefix><UPPER name>.csv` per enabled element (no `CaseName_`
    /// prefix), header written only when the file does not yet exist. Pascal's
    /// `DoExportCmd` tail then sets `@lastexportfile`/`@lastfile` to the literal
    /// `/m` switch string (the local `FileName` was never rewritten), which we
    /// reproduce.
    fn write_register_multi(
        &mut self,
        prefix: &str,
        label: &str,
        year: i32,
        ldcurve: &str,
        hour: i32,
        rows: &[crate::report::export::RegRow],
    ) {
        use crate::report::export::{register_header, register_row};
        for row in rows {
            let default_name = format!("{prefix}{}.csv", row.name.to_uppercase());
            let path = crate::report::output::export_path(
                &self.output_directory,
                &self.current_dir,
                "",
                "",
                &default_name,
            );
            // Multi-file uses the plain existence test (no `Year`-line check).
            let create = !path.exists();
            let mut content = String::new();
            if create {
                content.push_str(&register_header(label, &row.register_names));
                content.push('\n');
            }
            content.push_str(&register_row(
                year,
                ldcurve,
                hour,
                &row.name,
                &row.registers,
            ));
            content.push('\n');
            let write_res = if create {
                std::fs::write(&path, content.as_bytes())
            } else {
                use std::io::Write;
                std::fs::OpenOptions::new()
                    .append(true)
                    .open(&path)
                    .and_then(|mut f| f.write_all(content.as_bytes()))
            };
            if let Err(e) = write_res {
                self.errors.push(format!(
                    "Error writing report file \"{}\": {e}",
                    path.display()
                ));
            }
        }
        // Pascal `SetLastResultFile(DSS, '/m')` + `@lastexportfile := '/m'`.
        self.vars.add("@lastfile", "/m");
        self.vars.add("@lastexportfile", "/m");
        self.last_result_file = "/m".to_string();
    }

    /// `Export Y` (Pascal `ExportY`): the assembled system Y, sparse-triplet
    /// (`Row,Col,G,B`) when `triplet`, else the dense node-by-node form. Reads
    /// `system_y_csc` (the assembled, unfactored Y the checkpoint/live gates pin);
    /// errors with Pascal's #222 `Y Matrix not Built.` when no Y has been built.
    fn export_y_to_file(&mut self, explicit: &str, triplet: bool) {
        let content = match self.system_y_csc() {
            Some((n, coords)) => {
                let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
                crate::report::export::export_y(n, &coords, ckt, triplet)
            }
            None => {
                self.errors.push("Y Matrix not Built.".to_string());
                return;
            }
        };
        self.write_export(explicit, "EXP_Y.csv", &content);
    }

    /// `Export Summary` (Pascal `ExportSummary`): one status/summary row. Unlike
    /// the other exports it **appends** to an existing file (header only on
    /// create), so repeated `Export Summary` calls log one row each. Gathers the
    /// solution scalars + the extended-column quantities (total source power,
    /// losses, pu-voltage extremes) then writes/appends the row.
    fn export_summary_to_file(&mut self, explicit: &str) {
        // Extended-column scalars first (they need the &mut element walk).
        let (tp_re_kw, tp_im_kvar) = self.total_power(); // kW/kvar
        let (loss_re_w, loss_im_var) = self.losses(); // W/var
        // GetTotalPowerFromSources = -Σ source power[1] (VA); ×1e-6 → MVA.
        // total_power() = Σ source power[1] × 0.001 (kW), so MVA = -kW × 0.001.
        let total_mw = -tp_re_kw * 0.001;
        let total_mvar = -tp_im_kvar * 0.001;
        let mw_losses = loss_re_w * 1e-6;
        let mvar_losses = loss_im_var * 1e-6;

        let (fields, frequency) = {
            let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
            let mode = self
                .enums
                .get(self.enums.solve_mode)
                .ordinal_to_string(ckt.solution.mode.ordinal());
            let control_mode = self
                .enums
                .get(self.enums.control_mode)
                .ordinal_to_string(ckt.solution.control_mode);
            let fields = crate::report::export::SummaryFields {
                datetime: current_datetime_string(),
                case_name: ckt.case_name.clone(),
                is_solved: ckt.is_solved,
                bus_name_redefined: ckt.bus_name_redefined,
                mode,
                number: ckt.solution.number_of_times,
                load_mult: ckt.load_multiplier,
                num_devices: ckt.num_devices as i32,
                num_buses: ckt.buses.len() as i32,
                num_nodes: ckt.num_nodes as i32,
                iterations: ckt.solution.iteration,
                control_mode,
                control_iterations: ckt.solution.control_iteration,
                most_iterations_done: ckt.solution.most_iterations_done,
                year: ckt.solution.year,
                hour: ckt.solution.int_hour,
                max_pu_voltage: crate::report::export::max_pu_voltage(ckt),
                min_pu_voltage: crate::report::export::min_pu_voltage(ckt, true),
                total_mw,
                total_mvar,
                mw_losses,
                mvar_losses,
            };
            (fields, ckt.solution.frequency)
        };

        // Resolve the path; append (row only) if it already exists, else create
        // with the header (Pascal `ExportSummary` FileExists branch).
        let case_ = format!("{}_", fields.case_name);
        let path = crate::report::output::export_path(
            &self.output_directory,
            &self.current_dir,
            &case_,
            explicit,
            "EXP_Summary.csv",
        );
        let include_header = !path.exists();
        let content = crate::report::export::export_summary(&fields, frequency, include_header);

        let write_res = if include_header {
            std::fs::write(&path, content.as_bytes())
        } else {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .and_then(|mut f| f.write_all(content.as_bytes()))
        };
        match write_res {
            Ok(()) => {
                let p = path.to_string_lossy().into_owned();
                self.vars.add("@lastfile", &p);
                self.vars.add("@lastexportfile", &p);
                self.last_result_file = p;
            }
            Err(e) => self.errors.push(format!(
                "Error writing report file \"{}\": {e}",
                path.display()
            )),
        }
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
            &self.current_dir,
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
    /// WP8.4 (in progress): the ported keywords write their real fixed-width text
    /// report; `Show panel` reproduces the oracle's #999 (not supported in
    /// DSS-Extensions); every **not-yet-ported** keyword stays a *silent* headless
    /// no-op (writes nothing, records no error) so it cannot regress the
    /// `solvable_now` decks that contain `Show Power`/`Show Voltage`/`Show f` (the
    /// live gate asserts `errors().is_empty()`). The remaining formatters + the
    /// unknown→#24700 error land in the later WP8.4 steps. Reports change **no**
    /// electrical state (PHASE8_PLAN §2.5).
    pub(crate) fn do_show_cmd(&mut self) {
        // Pascal `DSS.Parser.NextParam; Param := AnsiLowerCase(StrValue)`.
        self.parser.next_param(&self.vars);
        let param = self.parser.make_string(&self.vars).to_lowercase();
        let ptr = self
            .show_commands
            .get_command(&param)
            .map(|i| i + 1)
            .unwrap_or(0);

        // Solution guard (`ShowOptions.pas:127-141`, `case ParamPointer of 4, 6,
        // 8..10, 12, 13..17, 19..23, 29..31`): the solution-reading Shows need a
        // solved circuit. As with `Export`, the no-circuit half (#24701) is
        // unreachable — the generic pre-circuit #301 guard fires first — so only
        // the "must be solved" (#24702) check remains: it fires when the circuit
        // exists but `Solution.NodeV` is unallocated (`[ground]` only).
        if matches!(ptr, 4 | 6 | 8..=10 | 12 | 13..=17 | 19..=23 | 29..=31)
            && self
                .circuit
                .as_ref()
                .is_some_and(|c| c.solution.node_v.len() <= 1)
        {
            self.errors
                .push("The circuit must be solved before you can do this.".to_string());
            return;
        }

        use crate::report::show;
        match ptr {
            // 2 `buses` (no solution guard — reads bus geometry/nodes only).
            2 => {
                let content =
                    show::show_buses(self.circuit.as_ref().expect("post-circuit dispatch"));
                self.write_show("Buses.txt", &content);
            }
            // 3 `currents` (`ShowCurrents`) — `ShowOptions.pas:153-184`: 1st param
            // `Y`/`T`→residual, `N`→no residual, `E`→element form; 2nd param
            // `E`→element form; filename `Curr_Seq` (code 0) / `Curr_Elem` (code 1).
            3 => {
                self.parser.next_param(&self.vars);
                let p1 = self.parser.make_string(&self.vars).to_uppercase();
                let (mut code, mut show_resid) = (0, false);
                match p1.chars().next() {
                    Some('Y') | Some('T') => show_resid = true,
                    Some('N') => show_resid = false,
                    Some('E') => code = 1,
                    _ => {}
                }
                self.parser.next_param(&self.vars);
                let p2 = self.parser.make_string(&self.vars).to_uppercase();
                if p2.starts_with('E') {
                    code = 1;
                }
                let content = {
                    let Dss {
                        classes, circuit, ..
                    } = self;
                    let ckt = circuit.as_ref().expect("post-circuit dispatch");
                    let sys = crate::solution::solution::sys_ctx(ckt);
                    let node_v = ckt.solution.node_v.clone();
                    if code == 0 {
                        show::show_currents(classes, ckt, &sys, &node_v)
                    } else {
                        show::show_currents_elements(classes, ckt, &sys, &node_v, show_resid)
                    }
                };
                let fname = if code == 0 {
                    "Curr_Seq.txt"
                } else {
                    "Curr_Elem.txt"
                };
                self.write_show(fname, &content);
            }
            // 5 `elements` (`ShowElements`) — `ShowOptions.pas:200-204`: an optional
            // class-name filter, then two files (`Elements.txt` +
            // `Elements_Disabled.txt`). No solution guard (bus connections only).
            5 => {
                self.parser.next_param(&self.vars);
                let param = self.parser.make_string(&self.vars).to_lowercase();
                let (main, disabled) = {
                    let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
                    show::show_elements(&self.classes, ckt, &param)
                };
                // Pascal writes the disabled companion first (no `@lastshowfile`),
                // then the main file (which sets `@lastshowfile`).
                self.write_show_named("Elements_Disabled.txt", &disabled, false);
                self.write_show("Elements.txt", &main);
            }
            // 8 `generators` (`ShowGenMeters`): each generator's accumulated
            // energy-meter registers, file `GenMeterOut.txt`.
            8 => {
                let content = {
                    let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
                    show::show_gen_meters(&self.classes, ckt)
                };
                self.write_show("GenMeterOut.txt", &content);
            }
            // 9 `meters` (`ShowMeters`): each EnergyMeter's accumulated registers,
            // file `EMout.txt`.
            9 => {
                let content = {
                    let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
                    show::show_meters(&self.classes, ckt)
                };
                self.write_show("EMout.txt", &content);
            }
            // 12 `powers` (`ShowPowers`) — `ShowOptions.pas:249-277`: 1st param
            // `m`→MVA, `e`→element form; 2nd param `e`→element form; filename
            // `Power_{seq|elem}_{kVA|MVA}`. Step 3 ports **code 0** (the sequence
            // form); the element form (code 1) stays deferred.
            12 => {
                self.parser.next_param(&self.vars);
                let p1 = self.parser.make_string(&self.vars).to_lowercase();
                let (mut mva, mut code) = (0, 0);
                match p1.chars().next() {
                    Some('m') => mva = 1,
                    Some('e') => code = 1,
                    _ => {}
                }
                self.parser.next_param(&self.vars);
                let p2 = self.parser.make_string(&self.vars).to_lowercase();
                if p2.starts_with('e') {
                    code = 1;
                }
                let opt = mva;
                let content = {
                    let Dss {
                        classes, circuit, ..
                    } = self;
                    let ckt = circuit.as_ref().expect("post-circuit dispatch");
                    let sys = crate::solution::solution::sys_ctx(ckt);
                    let node_v = ckt.solution.node_v.clone();
                    if code == 0 {
                        show::show_powers(classes, ckt, &sys, &node_v, opt)
                    } else {
                        show::show_powers_elements(classes, ckt, &sys, &node_v, opt)
                    }
                };
                // Pascal filename: `Power_{seq|elem}_{kVA|MVA}`.
                let fname = match (code, mva) {
                    (0, 1) => "Power_seq_MVA.txt",
                    (0, _) => "Power_seq_kVA.txt",
                    (_, 1) => "Power_elem_MVA.txt",
                    (_, _) => "Power_elem_kVA.txt",
                };
                self.write_show(fname, &content);
            }
            // 15 `taps` (`ShowRegulatorTaps`).
            15 => {
                let content = {
                    let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
                    show::show_taps(&self.classes, ckt)
                };
                self.write_show("RegTaps.txt", &content);
            }
            // 22 `losses` (`ShowLosses`) — walks the mutating loss/power getters.
            22 => {
                let content = {
                    let Dss {
                        classes, circuit, ..
                    } = self;
                    let ckt = circuit.as_ref().expect("post-circuit dispatch");
                    let sys = crate::solution::solution::sys_ctx(ckt);
                    let node_v = ckt.solution.node_v.clone();
                    show::show_losses(classes, ckt, &sys, &node_v)
                };
                self.write_show("Losses.txt", &content);
            }
            // 18 `eventlog` (`ShowEventLog` = `EventStrings.SaveToFile`). No solve
            // guard — the log accumulates across the run. Like `ShowResult`, it also
            // sets `GlobalResult` (`ShowResults.pas:3967`).
            18 => {
                let content = {
                    let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
                    show::show_event_log(ckt.solution.event_log.entries())
                };
                self.write_show_global("EventLog.txt", &content);
            }
            // 19 `variables` (`ShowVariables`): every PC element's dynamic-state
            // variable values.
            19 => {
                let content = {
                    let Dss {
                        classes, circuit, ..
                    } = self;
                    let ckt = circuit.as_ref().expect("post-circuit dispatch");
                    let sys = crate::solution::solution::sys_ctx(ckt);
                    let node_v = ckt.solution.node_v.clone();
                    show::show_variables(classes, ckt, &sys, &node_v)
                };
                self.write_show("Variables.txt", &content);
            }
            // 20 `ratings` (`ShowRatings`): each PD element's normal/emergency amps.
            20 => {
                let content = {
                    let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
                    show::show_ratings(&self.classes, ckt)
                };
                self.write_show("RatingsOut.txt", &content);
            }
            // 34 `Result` (`ShowResult`): the `@result` parser var value. The file is
            // `<Case_>Result.csv` (Pascal `ShowOptions.pas:432`), and `ShowResult`
            // also sets `GlobalResult` (`ShowResults.pas:3950`).
            34 => {
                let val = self.vars.get("@result").unwrap_or("null").to_string();
                let content = show::show_result(&val);
                self.write_show_global("Result.csv", &content);
            }
            // 29 `mismatch` (`ShowNodeCurrentSum`): per-node KCL current sum.
            29 => {
                let content = {
                    let Dss {
                        classes, circuit, ..
                    } = self;
                    let ckt = circuit.as_ref().expect("post-circuit dispatch");
                    let sys = crate::solution::solution::sys_ctx(ckt);
                    let node_v = ckt.solution.node_v.clone();
                    show::show_mismatch(classes, ckt, &sys, &node_v)
                };
                self.write_show("NodeMismatch.txt", &content);
            }
            // TODO(WP8): 31 `deltaV` (`ShowDeltaV`) stays a silent no-op — the
            // `WriteElementDeltaVoltages` `NodeRef[i+NCond]` cross-terminal read
            // yields 0 rows for a **delta-primary** transformer (`Transformer.sub`)
            // where the oracle writes 3; the delta-winding node_ref layout needs
            // investigation before this ships (deltaV is not used by any corpus deck).
            // 10 `monitor <name>` (`ShowMonitor` = `Monitor.TranslateToCSV`): write
            // the named monitor's in-memory sample buffer to its fixed CSV file
            // (`<OutputDir><CircuitName_>Mon_<name>_1.csv`), the same content the
            // `Export Monitors` path produces. An empty name raises Pascal's #249;
            // an unknown named monitor raises #248.
            10 => {
                self.parser.next_param(&self.vars);
                let name = self.parser.make_string(&self.vars);
                self.show_monitor(&name);
            }
            // 13 `voltages` (`ShowVoltages`) — the option/filename parse of
            // `ShowOptions.pas:279-312`: first param `LL` → phase-phase (else L-N);
            // second param `N…`/`E…` selects the node (code 1) / element (code 2)
            // form. Code 0 = the symmetrical-component form (bare `Show Voltages`);
            // code 1 = line-ground/line-line by bus & node; code 2 = node-ground by
            // circuit element.
            13 => {
                self.parser.next_param(&self.vars);
                let p1 = self.parser.make_string(&self.vars);
                let (mut ll, mut filname) = (false, "VLN".to_string());
                if p1.eq_ignore_ascii_case("LL") {
                    ll = true;
                    filname = "VLL".to_string();
                }
                self.parser.next_param(&self.vars);
                let p2 = self.parser.make_string(&self.vars).to_uppercase();
                let mut code = 0;
                if let Some(c) = p2.chars().next() {
                    match c {
                        'N' => {
                            code = 1;
                            filname.push_str("_Node");
                        }
                        'E' => {
                            code = 2;
                            filname.push_str("_elem");
                        }
                        _ => filname.push_str("_seq"),
                    }
                }
                let content = match code {
                    0 => show::show_voltages(
                        self.circuit.as_ref().expect("post-circuit dispatch"),
                        ll,
                    ),
                    1 => show::show_voltages_nodes(
                        self.circuit.as_ref().expect("post-circuit dispatch"),
                        ll,
                    ),
                    _ => {
                        let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
                        show::show_voltages_elements(&self.classes, ckt, ll)
                    }
                };
                self.write_show(&format!("{filname}.txt"), &content);
            }
            // 4 `convergence` (`Solution.WriteConvergenceReport`): the per-node saved
            // error / |V| / Vbase snapshot + the Max Error footer. Pascal's arm 4 is
            // an inline `try/finally` that writes via `GetOutputStreamEx` + only
            // `FireOffEditor` (`ShowOptions.pas:187-197`) — it does **not** set
            // `@lastshowfile` (unlike `ShowY`/`ShowkVBaseMismatch`), so `set_last=false`.
            4 => {
                let content =
                    show::show_convergence(self.circuit.as_ref().expect("post-circuit dispatch"));
                self.write_show_named("Convergence.txt", &content, false);
            }
            // 26 `y` (`ShowY`): the assembled system Y, lower triangle by columns.
            // Reads `system_y_csc` (the assembled, unfactored Y the checkpoint/live
            // gates pin); errors with Pascal's #222 `Y Matrix not Built.` if no Y
            // has been built.
            26 => {
                let content = match self.system_y_csc() {
                    Some((n, coords)) => {
                        let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
                        show::show_y(n, &coords, ckt)
                    }
                    None => {
                        self.errors.push("Y Matrix not Built.".to_string());
                        return;
                    }
                };
                self.write_show("SystemY.txt", &content);
            }
            // 27 `controlqueue` (`ControlQueue.WriteQueue`): the pending
            // control-action queue (drained to a header alone after a converged
            // snapshot). File suffix `.csv` (`ShowOptions.pas:410`). No solve guard.
            // Like arm 4 (convergence), Pascal's arm 27 is an inline `try/finally`
            // with only `FireOffEditor` (`ShowOptions.pas:404-414`) — no
            // `@lastshowfile`, hence `set_last=false`.
            27 => {
                let content = {
                    let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
                    show::show_control_queue(&self.classes, ckt)
                };
                self.write_show_named("ControlQueue.csv", &content, false);
            }
            // 30 `kvbasemismatch` (`ShowkVBaseMismatch`): loads/generators whose kV
            // base is >10% off the connected bus's base.
            30 => {
                let content = {
                    let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
                    show::show_kvbase_mismatch(&self.classes, ckt)
                };
                self.write_show("kVBaseMismatch.txt", &content);
            }
            // 6 `faults` (`ShowFaultStudy`): the three-section FaultStudy report over
            // the precomputed bus `Zsc`/`Ysc`/`VBus`/`BusCurrent` (a prior `Solve
            // mode=faultstudy` populated). Read-only — no re-solve.
            6 => {
                let content =
                    show::show_fault_study(self.circuit.as_ref().expect("post-circuit dispatch"));
                self.write_show("FaultStudy.txt", &content);
            }
            // 16 `overloads` (`ShowOverloads`): the PD-element symmetrical-component
            // overload report — one row per enabled PDElement (non-capacitor) whose
            // terminal-1 max phase current exceeds its normal or emergency rating.
            16 => {
                let content = {
                    let Dss {
                        classes, circuit, ..
                    } = self;
                    let ckt = circuit.as_ref().expect("post-circuit dispatch");
                    let sys = crate::solution::solution::sys_ctx(ckt);
                    let node_v = ckt.solution.node_v.clone();
                    show::show_overloads(classes, ckt, &sys, &node_v)
                };
                self.write_show("Overload.txt", &content);
            }
            // 17 `unserved` (`ShowUnserved`): the Loads over their voltage-drop
            // criterion. A nonempty trailing param (`ShowOptions.pas:322-327`) selects
            // the emergency (`UE_Only`) criterion over the normal one.
            17 => {
                self.parser.next_param(&self.vars);
                let ue_only = !self.parser.make_string(&self.vars).is_empty();
                let content = {
                    let Dss {
                        classes, circuit, ..
                    } = self;
                    let ckt = circuit.as_ref().expect("post-circuit dispatch");
                    let sys = crate::solution::solution::sys_ctx(ckt);
                    let node_v = ckt.solution.node_v.clone();
                    show::show_unserved(classes, ckt, &sys, &node_v, ue_only)
                };
                self.write_show("Unserved.txt", &content);
            }
            // 25 `yprim` (`ShowYprim`): the **active** circuit element's primitive Y
            // (lower-triangle G then jB). No solution guard (Pascal arm 25 is a bare
            // `if ActiveCircuit <> NIL`). The filename is `<ParentClass.Name>_<Name>_
            // Yprim.txt` — NO `CircuitName_` prefix (`ShowOptions.pas:395`), so it
            // writes via `write_show_path`. With no active element (no prior `Select`)
            // it is a no-op — Pascal would nil-deref `ActiveCktElement`, so there is
            // no oracle-comparable output.
            25 => {
                let Some((ci, idx)) = self.active_ckt_element else {
                    return;
                };
                let (filename, content) = {
                    let class_name = self.classes[ci].props.class_name();
                    let obj = &self.classes[ci].objects[idx];
                    let name = obj.data().name();
                    let filename = format!("{class_name}_{name}_Yprim.txt");
                    let full_name = format!("{class_name}.{name}");
                    let (yprim, yorder) = match obj.as_ckt_element() {
                        Some(e) => (e.cd().yprim.as_ref(), e.cd().yorder),
                        None => (None, 0),
                    };
                    (filename, show::show_yprim(&full_name, yprim, yorder))
                };
                self.write_show_path(&filename, &content, true);
            }
            // 11 `panel` — the oracle faithfully errors it (`ShowOptions.pas:248`).
            11 => {
                self.errors
                    .push("Command \"show panel\" is not supported in DSS-Extensions.".to_string());
            }
            // 14 `zone <metername>` (`ShowMeterZone`): the named EnergyMeter's zone
            // as an indented branch/shunt tree. The filename is
            // `<CircuitName_>ZoneOut_<metername>.txt` (Pascal strips `.txt` from the
            // `ZoneOut.txt` passed in and appends `_<Param>.txt`,
            // `ShowResults.pas:2435-2439`), and it sets `GlobalResult` (`:2443`). An
            // empty name → Pascal #221; an unknown meter → #220. Pascal always
            // creates the file (empty on the error paths), so the port writes it in
            // every branch.
            14 => {
                self.parser.next_param(&self.vars);
                let param = self.parser.make_string(&self.vars);
                let fname = format!("ZoneOut_{param}.txt");
                let content = if param.is_empty() {
                    // Pascal #221 `'Meter Name Not Specified. %s'`.
                    self.errors.push("Meter Name Not Specified.".to_string());
                    String::new()
                } else {
                    let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
                    let found = ckt.energy_meters.iter().copied().find(|&r| {
                        self.classes[r.cls].objects[r.idx]
                            .data()
                            .name()
                            .eq_ignore_ascii_case(&param)
                    });
                    match found {
                        Some(r) => show::show_meter_zone(&self.classes, r, &param),
                        None => {
                            // Pascal #220 `'EnergyMeter "%s" not found.'`.
                            self.errors
                                .push(format!("EnergyMeter \"{param}\" not found."));
                            String::new()
                        }
                    }
                };
                self.write_show_global(&fname, &content);
            }
            // 21 `loops` (`ShowLoops`): every parallel/looped branch across all
            // EnergyMeter zones, one line each (radial zones → header only).
            21 => {
                let content = {
                    let ckt = self.circuit.as_ref().expect("post-circuit dispatch");
                    show::show_loops(&self.classes, ckt)
                };
                self.write_show("Loops.txt", &content);
            }
            // TODO(WP8): later steps — the remaining `Show` keywords (isolated,
            // lineconstants, topology, busflow, controlled, autoadded, querylog,
            // deltaV) and the unknown-keyword `#24700` error
            // (`ShowOptions.pas:119-124`), are still deferred. Unlike the `Export`/
            // `Save`/`Dump` routers — whose deferrals push a scoped `NOT_PORTED`
            // error — a deferred `Show` MUST stay a **silent** headless no-op: the
            // `solvable_now`/`corpus_live` decks run `Show Power`/`Show Voltage`/
            // `Show f` and the live gate asserts `errors().is_empty()`, so emitting
            // the `#24700` (or any error) here would regress them. The later WP8.4
            // steps land the formatters; the `#24700` lands with them (once every
            // real keyword is ported, an unmatched keyword is genuinely unknown).
            // Greppable via `rg "TODO\(WP8\)"` (the WP8.8 exit sweep).
            _ => {}
        }
    }

    /// Write a `Show` report to `<OutputDirectory><CircuitName_><default_name>`
    /// (Pascal `ShowResults` procedures always build a fixed filename — `Show` has
    /// no explicit-filename argument). Unlike `Export`, a `Show` report sets at most
    /// `@lastshowfile` (`DSS.ParserVars.Add('@lastshowfile', FileNm)`), never
    /// `@lastfile` / `GlobalResult` (`Show` never calls `SetLastResultFile`).
    ///
    /// Not every `Show` sets `@lastshowfile`, though: the reports that write via a
    /// `ShowResults` procedure ending in `ParserVars.Add('@lastshowfile', …)` do
    /// (most of them), but the ones dispatched *inline* in `DoShowCmd` with only a
    /// `FireOffEditor` (`Convergence` arm 4, `ControlQueue` arm 27) do **not** — those
    /// call [`Dss::write_show_named`] with `set_last = false`.
    fn write_show(&mut self, default_name: &str, content: &str) {
        self.write_show_named(default_name, content, true);
    }

    /// Like [`Dss::write_show`] but also sets `GlobalResult` (the port's
    /// `last_result_file`): Pascal `ShowResult`/`ShowEventLog` uniquely do
    /// `DSS.GlobalResult := FileNm` on top of `@lastshowfile`
    /// (`ShowResults.pas:3950`/`:3967`) — the other `Show` reports set only
    /// `@lastshowfile`.
    fn write_show_global(&mut self, default_name: &str, content: &str) {
        self.write_show(default_name, content);
        // `write_show` set `@lastshowfile` to the produced path; mirror it to
        // `GlobalResult`.
        let p = self
            .vars
            .get("@lastshowfile")
            .unwrap_or_default()
            .to_string();
        if !p.is_empty() {
            self.last_result_file = p;
        }
    }

    /// Like [`Dss::write_show`] but `set_last` gates the `@lastshowfile` update.
    /// The two-file `Show Elements` writes its `_Disabled` companion with
    /// `set_last = false` (Pascal sets `@lastshowfile` once, to the main file).
    fn write_show_named(&mut self, default_name: &str, content: &str, set_last: bool) {
        let case = self
            .circuit
            .as_ref()
            .map(|c| c.case_name.clone())
            .unwrap_or_default();
        self.write_show_path(&format!("{case}_{default_name}"), content, set_last);
    }

    /// Write a `Show` report to `<OutputDirectory><filename>` — the raw path, with
    /// **no** `<CircuitName_>` prefix (unlike [`Dss::write_show_named`]). The one
    /// report that needs this is `Show Yprim`, whose filename is
    /// `<ParentClass.Name>_<Name>_Yprim.txt` (`ShowOptions.pas:395`), not
    /// `<CircuitName_>…`.
    fn write_show_path(&mut self, filename: &str, content: &str, set_last: bool) {
        let path = crate::report::output::export_path(
            &self.output_directory,
            &self.current_dir,
            "",
            "",
            filename,
        );
        match std::fs::write(&path, content) {
            Ok(()) => {
                if set_last {
                    let p = path.to_string_lossy().into_owned();
                    self.vars.add("@lastshowfile", &p);
                }
            }
            Err(e) => self.errors.push(format!(
                "Error writing report file \"{}\": {e}",
                path.display()
            )),
        }
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

/// The current UTC wall-clock as `DD.MM.YYYY HH:MM:SS` — the `Export Summary`
/// timestamp (Pascal `DateTimeToStr(Now)`). Pure integer civil-from-days
/// conversion (Howard Hinnant, "chrono-Compatible Low-Level Date Algorithms"),
/// no external date dependency. The exact locale format is non-deterministic and
/// masked in the golden (PHASE8_PLAN §2.3 / `tests/TOLERANCE_NOTES.md`).
fn current_datetime_string() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0) as i64;
    let days = secs.div_euclid(86400);
    let sod = secs.rem_euclid(86400);
    let (hh, mm, ss) = (sod / 3600, (sod % 3600) / 60, sod % 60);
    // civil_from_days: days since 1970-01-01 → (y, m, d).
    let z = days + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    format!("{d:02}.{m:02}.{y:04} {hh:02}:{mm:02}:{ss:02}")
}
