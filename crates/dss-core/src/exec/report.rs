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
    match std::fs::read_to_string(path) {
        Ok(s) => !s
            .lines()
            .next()
            .and_then(|l| l.get(..4))
            .is_some_and(|p| p.eq_ignore_ascii_case("Year")),
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
        // (Pascal `ExportOptions.pas:190-298`). In this WP only `Powers`(9) /
        // `P_byphase`(19) consume one — the MVA/kVA flag (`m…` → MVA, else kVA).
        // The other Parm2-consuming reports (8 UE-only / 15 monitor name / 17
        // triplet flag / 20-21 CIM / 32 profile phases / 51 meter) land in later
        // WPs; each must likewise pre-parse here, ahead of the filename read, so
        // its `Parm2` is not mis-consumed as the filename.
        let mut mva_opt = 0;
        if matches!(ptr, 9 | 19) {
            self.parser.next_param(&self.vars);
            let parm2 = self.parser.make_string(&self.vars).to_lowercase();
            if parm2.starts_with('m') {
                mva_opt = 1;
            }
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
