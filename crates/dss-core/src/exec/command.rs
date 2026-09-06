//! The command dispatcher (`ProcessCommand`) plus object lifecycle and
//! editing (`New`/`Edit`/`~`/`Clear`/`?` and `TDSSClass.Edit`).
//! Split out of `exec/mod.rs`.

use super::*;

/// Run `RecalcElementData` against `sys` for the PC classes whose recalc consumes
/// the live `ActiveCircuit.Solution` globals (Load/Generator/WindGen/Storage/
/// PVSystem/IndMach012). Pascal `T<PC>Obj.Create` ends with this recalc; it is
/// the Rust equivalent of "Create reads the live solution", invoked by the
/// executive right after construction (it has the circuit; the PC `new` does not).
/// No-op for every other class. Reads only own props + `sys` (never another
/// element), so it is safe on the JSON pre-fill and on the defaults sample.
pub(super) fn recalc_pc_create(
    arena: &mut crate::obj::arena::ClassArena,
    idx: usize,
    sys: &crate::elements::traits::SysCtx,
) {
    if let Some(e) = arena.get_mut::<load::Load>(idx) {
        e.recalc(sys);
    } else if let Some(e) = arena.get_mut::<generator::Generator>(idx) {
        e.recalc(sys);
    } else if let Some(e) = arena.get_mut::<windgen::WindGen>(idx) {
        e.recalc(sys);
    } else if let Some(e) = arena.get_mut::<storage::Storage>(idx) {
        e.recalc(sys);
    } else if let Some(e) = arena.get_mut::<pvsystem::PVSystem>(idx) {
        e.recalc(sys);
    } else if let Some(e) = arena.get_mut::<ind_mach012::IndMach012>(idx) {
        e.recalc(sys);
    }
}

impl Dss {
    /// Process one command line entered **from outside** the engine — the
    /// library's public command entry, i.e. CAPI `Text_Set_Command`
    /// (`CAPI_Text.pas:35`): the "Reset for commands entered from outside"
    /// abort clear, then Pascal `ProcessCommand` ([`Self::process_command`]).
    /// Errors are recorded in [`Dss::errors`] (record-and-continue); query
    /// results land in [`Dss::result`].
    pub fn command(&mut self, cmd_line: &str) {
        if !self.in_redirect {
            // CAPI `Text_Set_Command`: "Reset for commands entered from
            // outside" — a previous abort doesn't poison the next external
            // command, while a Redirect/Compile still aborts the rest of its
            // file (`do_redirect` checks the live flag between lines).
            if let Some(ckt) = self.circuit.as_mut() {
                ckt.solution.solution_abort = false;
            }
        }
        self.process_command(cmd_line);
    }

    /// Pascal `ProcessCommand` (`ExecCommands.pas:214`) — the executive's own
    /// command processor, which is what `TExecutive.ParseCommand`
    /// (`Executive.pas:225`) calls and therefore what an *engine-internal*
    /// nested command (`DoEstimateCmd`'s two tail commands, …) runs. It resets
    /// only `CmdResult`/`ErrorNumber`/`GlobalResult`; the `SolutionAbort` clear
    /// above belongs to the outside-entry wrapper alone — upstream the line
    /// `SolutionAbort := FALSE  // Reset for commands entered from outside`
    /// occurs 32× and only under `src/CAPI/*`, and the four engine-side resets
    /// (`Circuit.pas:1607`, `Diakoptics.pas:546/703`,
    /// `DSSCallBackRoutines.pas:152`) are off the command path.
    pub(crate) fn process_command(&mut self, cmd_line: &str) {
        self.last_result.clear(); // DSS.GlobalResult := ''
        self.parser.set_auto_increment(false);
        self.parser.set_cmd_string(cmd_line);
        let param_name = self.parser.next_param(&self.vars);
        let param = self.parser.make_string(&self.vars);
        if param.is_empty() {
            return; // Skip blank line
        }

        // Commands do not have equal signs, so ParamName must be empty.
        let pointer = if param_name.is_empty() {
            self.commands
                .get_command(&param)
                .map(|i| i + 1)
                .unwrap_or(0)
        } else {
            0
        };

        // Check first for Compile or Redirect and get outta here.
        if pointer == cmd::COMPILE || pointer == cmd::REDIRECT {
            self.do_redirect(pointer == cmd::COMPILE);
            return;
        }

        // A-Diakoptics tearing commands (`Tear_Circuit`, `AggregateProfiles`)
        // are compiled out of the vendored/oracle build (plan §0.2), so they are
        // absent from `EXEC_COMMANDS` (which the oracle-pinned `Dump commands`
        // golden mirrors byte-exact). Register them here as a deliberate,
        // recorded departure — the engine behaves like a `DSS_CAPI_ADIAKOPTICS`
        // build — without perturbing that golden. `help` still resolves them
        // (`help_catalog` already carries their text). See STATUS §WP-AD.2.
        if pointer == 0 && param_name.is_empty() {
            match param.to_ascii_lowercase().as_str() {
                "tear_circuit" => {
                    if self.circuit.is_none() {
                        self.errors.push(
                            "You must create a new circuit object first: \"new circuit.mycktname\" to execute this command."
                                .to_string(),
                        );
                    } else {
                        self.do_tear_circuit_cmd();
                    }
                    return;
                }
                "aggregateprofiles" => {
                    // NOT_PORTED (scoped): `AggregateProfiles` is WP-AD.5.
                    self.errors.push(
                        "Command \"AggregateProfiles\" is not ported yet (WP-AD.5).".to_string(),
                    );
                    return;
                }
                _ => {}
            }
        }

        // Things that are OK to do before a circuit is defined.
        match pointer {
            cmd::NEW => {
                self.do_new_cmd();
                return;
            }
            cmd::SET if self.circuit.is_none() => {
                self.do_set_cmd_no_circuit();
                return;
            }
            cmd::GET if self.circuit.is_none() => {
                self.do_get_cmd_no_circuit();
                return;
            }
            cmd::COMMENT | cmd::HELP | cmd::QUIT | cmd::PANEL | cmd::ABOUT | cmd::COMHELP => {
                return; // no-ops (comment / GUI-only commands)
            }
            cmd::CLEAR | cmd::CLEAR_ALL => {
                self.do_clear_cmd();
                return;
            }
            // Pre-circuit commands (`ExecCommands.pas:301-335`, dispatched
            // before the circuit-required gate; the second, post-circuit case
            // carries only commented-out duplicates of these).
            cmd::FILEEDIT => {
                self.do_file_edit_cmd();
                return;
            }
            cmd::CLASSES => {
                // Pascal `DoClassesCmd`: every intrinsic class name into
                // GlobalResult, in `DSSClassList` creation order (the Rust
                // registry groups DSS_OBJECT classes first, so walk the
                // Pascal-order table the whole-circuit Dump already uses).
                for name in crate::report::save::dump::commands::PASCAL_CLASS_ORDER {
                    super::helpers::append_result(&mut self.last_result, name);
                }
                return;
            }
            cmd::USERCLASSES => {
                // Pascal `DoUserClassesCmd`.
                super::helpers::append_result(&mut self.last_result, "No User Classes Defined.");
                return;
            }
            cmd::CD => {
                self.do_cd_cmd();
                return;
            }
            cmd::DOSCMD => {
                // Pascal `ExecCommands.pas:327`: `DSS_CAPI_ALLOW_DOSCMD`
                // defaults off and stays off in this port (arbitrary shell
                // execution; the enabling API is deliberately not exposed) —
                // the error #283 arm is the whole surface.
                self.errors.push(crate::diag::DssDiagnostic::msg(
                    "DOScmd is disabled. Enable it via API or set the environment variable DSS_CAPI_ALLOW_DOSCMD=1 before starting the process.",
                    Some(283),
                ));
                return;
            }
            cmd::VAR => {
                self.do_var_cmd();
                return;
            }
            // `AlignFile`/`CvrtLoadshapes` are live upstream but unexercised
            // file-rewrite utilities (0 corpus uses) — loud NOT_PORTED,
            // on-demand owners. The DI-plot family (`DI_plot`/`CompareCases`/
            // `YearlyCurves`) stays loud: upstream calls the plot callback
            // with no NIL guard (UB when unregistered — the WPG.17 rule).
            cmd::ALIGN_FILE
            | cmd::DI_PLOT
            | cmd::COMPARE_CASES
            | cmd::YEARLY_CURVES
            | cmd::CVRT_LOADSHAPES => {
                self.not_ported_command(pointer);
                return;
            }
            0 => {} // possibly a property reference; checked below
            _ => {
                if self.circuit.is_none() {
                    self.errors.push(
                        "You must create a new circuit object first: \"new circuit.mycktname\" to execute this command."
                            .to_string(),
                    );
                    return;
                }
            }
        }

        // Not a command: it could be a property of the active circuit element.
        if pointer == 0 {
            if param_name.is_empty() || param_name.eq_ignore_ascii_case("command") {
                let from = self.errors.len();
                self.errors.push(format!("Unknown Command: \"{param}\""));
                // P5b: underline the unknown command name (the value token).
                attach_source(
                    &mut self.errors,
                    from,
                    self.parser.token_span(),
                    &self.cmd_origin,
                    self.parser.cmd_string(),
                );
            } else {
                let (obj_name, prop_name) = parse_obj_name(&param_name);
                if !obj_name.is_empty() && !self.set_object(&obj_name) {
                    return;
                }
                // Rebuild the command line and pass it to the editor; quotes
                // ensure the first parameter is interpreted OK after rebuild.
                let remainder = self.parser.remainder().to_string();
                self.parser
                    .set_cmd_string(&format!("{prop_name}=\"{param}\" {remainder}"));
                self.edit_active();
            }
            return;
        }

        // Process the rest of the commands (circuit exists at this point).
        match pointer {
            cmd::EDIT => self.do_edit_cmd(),
            cmd::MORE | cmd::M | cmd::TILDE => self.edit_active(),
            cmd::SELECT => self.do_select_cmd(),
            cmd::OPEN => self.do_open_close_cmd(false),
            cmd::CLOSE => self.do_open_close_cmd(true),
            // Solve = Set + DoSolveCmd. `SolveAll` (`Solve all`) is the
            // PM-build command that solves every actor; with a single actor
            // it reduces to a plain solve of the active circuit
            // (`ExecCommands.pas:346`).
            cmd::SOLVE | cmd::SOLVE_ALL => self.do_set_cmd(1),
            cmd::SET => self.do_set_cmd(0),
            cmd::QUERY => self.do_query_cmd(),
            cmd::CALC_VOLTAGE_BASES => self.do_calc_voltage_bases(),
            cmd::BUILD_Y => self.do_build_y(),
            cmd::GET => self.do_get_cmd(),
            cmd::SAMPLE => self.do_sample_cmd(),
            // Pascal `DoCloseDICmd` (`ExecHelper.pas:4199`): flush + close any
            // open demand-interval files.
            cmd::CLOSE_DI => self.do_close_di_cmd(),
            cmd::RESET => self.do_reset_cmd(),
            cmd::ALLOCATE_LOADS => self.do_allocate_loads_cmd(),
            // Pascal `DoEstimateCmd` (`ExecHelper.pas:4213`): allocate, then
            // export the estimation report.
            cmd::ESTIMATE => self.do_estimate_cmd(),
            cmd::DISTRIBUTE => self.do_distribute_cmd(),
            cmd::UUIDS => self.do_uuids_cmd(),
            cmd::RELCALC => self.do_relcalc_cmd(),
            cmd::REDUCE => self.do_reduce_cmd(),
            cmd::BUSCOORDS => self.do_bus_coords_cmd(false),
            // Pascal `DoBusCoordsCmd(TRUE)` — the swap-XY (Lat/Lon) variant; the
            // implementation is shared with BusCoords (WPG.16 wires the dispatch
            // so the GICExample corpus deck's trailing `LatLongCoords` runs).
            cmd::LATLONGCOORDS => self.do_bus_coords_cmd(true),
            cmd::EXPORT => self.do_export_cmd(),
            cmd::SAVE => self.do_save_cmd(),
            cmd::DUMP => self.do_dump_cmd(),
            // Pascal `DoShowCmd` (ShowOptions.pas). Post-circuit dispatch (Pascal
            // `ProcessCommand`), so `Show` before a circuit falls through to the
            // generic guard above and records the #301 "create a circuit first"
            // error exactly like the oracle (audit-code WP8.1: the oracle's
            // dispatch gate errors #301 before `DoShowCmd` runs). With a circuit,
            // every `Show` report is a faithful headless no-op (writes a file,
            // changes no electrical state — PHASE8_PLAN §2.5); WP8.4 lands the
            // real ShowResults formatters + the 24700/24701/24702/999 errors.
            cmd::SHOW => self.do_show_cmd(),
            // Pascal `DoPlotCmd` (`PlotOptions.pas:182`). With no plot callback
            // registered it exits before ANY guard (the pinned headless oracle),
            // so `Plot` is a faithful total no-op; with a callback registered
            // (WPG.17) it parses the options and fires the `plotParams` JSON.
            // Post-circuit, like the oracle's dispatch (before a circuit the
            // generic guard above emits #301; oracle-probed).
            cmd::PLOT => self.do_plot_cmd(),
            // Pascal `DoAddMarkerCmd` / `TDSSCircuit.ClearBusMarkers`: the
            // bus-marker list feeding the plot payload's `BusMarkers[]` (WPG.17).
            cmd::ADD_BUS_MARKER => self.do_add_marker_cmd(),
            cmd::CLEAR_BUS_MARKER => self.do_clear_bus_markers_cmd(),
            // `Visualize` differs: `DoVisualizeCmd` runs its guards BEFORE the
            // callback check, so they are engine-observable and ported.
            cmd::VISUALIZE => self.do_visualize_cmd(),
            cmd::BATCH_EDIT => self.do_batch_edit_cmd(),
            // Pascal `ExecCommands.pas` `ord(Cmd.MakeBusList)`:
            // `if BusNameRedefined then ReprocessBusDefs` — nothing else.
            cmd::MAKE_BUS_LIST => self.do_make_bus_list_cmd(),
            // Pascal `ExecCommands.pas` `ord(Cmd.GISCoords)`: "Do nothing here
            // on DSS C-API. Just ignore it silently so files saved with EPRI's
            // version can be loaded more easily." (OpenDSS-GIS is out of the
            // DSS-Extensions scope.)
            cmd::GIS_COORDS => {}
            // Pascal `ExecCommands.pas` `ord(Cmd.Wait)`: `if
            // PMParent.Parallel_enabled then Wait4Actors(...)` — with the
            // parallel machinery disabled (the pinned oracle's default and this
            // port's only mode until MULTITHREADING_PLAN lands actors) it is a
            // silent no-op, exactly like the oracle (the StoCtrl corpus deck's
            // `Add_Issues.dss` issues a bare `wait` mid-script).
            cmd::WAIT => {}
            cmd::SET_BUS_XY => self.do_set_bus_xy_cmd(),
            cmd::INTERPOLATE => self.do_interpolate_cmd(),
            // Pascal `DoRemoveCmd` (ExecHelper.pas:4939) → `DoRemoveBranches`.
            cmd::REMOVE => self.do_remove_cmd(),
            cmd::INIT => {
                if let Some(ckt) = self.circuit.as_mut() {
                    ckt.solution.solution_initialized = false;
                }
            }
            // Pascal `DoEnableCmd`/`DoDisableCmd` (ExecHelper.pas:1095/1145).
            cmd::ENABLE => self.do_enable_disable_cmd(true),
            cmd::DISABLE => self.do_enable_disable_cmd(false),
            // Pascal `DoSetkVBase` (ExecHelper.pas:1949).
            cmd::SET_KV_BASE => self.do_set_kv_base_cmd(),
            // Pascal `DolossesCmd` (ExecHelper.pas:2168).
            cmd::LOSSES => self.do_losses_cmd(),
            // Pascal `DoSummaryCmd` (ExecHelper.pas:3449).
            cmd::SUMMARY => self.do_summary_cmd(),
            // Pascal `DoReconductorCmd` (ExecHelper.pas:4245).
            cmd::RECONDUCTOR => self.do_reconductor_cmd(),
            // Pascal `DoPstCalc` (ExecHelper.pas:4778).
            cmd::PSTCALC => self.do_pst_calc_cmd(),
            // The step-solution commands (`ExecCommands.pas:578-601`): direct
            // drivers over the solution internals.
            cmd::INIT_SNAP
            | cmd::SOLVE_NO_CONTROL
            | cmd::SAMPLE_CONTROLS
            | cmd::DO_CONTROL_ACTIONS
            | cmd::SHOW_CONTROL_QUEUE
            | cmd::SOLVE_DIRECT
            | cmd::SOLVE_PFLOW => self.do_step_solution_cmd(pointer),
            // Incidence matrix commands (WP-AD.1, Pascal `ExecCommands.pas:406-433`).
            cmd::CALC_INC_MATRIX => self.do_calc_inc_matrix(false),
            cmd::CALC_INC_MATRIX_O => self.do_calc_inc_matrix(true),
            cmd::REFINE_BUSLEVELS => self.do_refine_bus_levels(),
            cmd::CALC_LAPLACIAN => self.do_calc_laplacian(),
            // Pascal `TExecHelper.DoMakePosSeq` (ExecHelper.pas:3035): flip the
            // circuit to positive-sequence and convert every element in creation
            // order (`exec/make_pos_seq.rs`).
            cmd::MAKE_POS_SEQ => self.do_make_pos_seq(),
            _ => self.not_ported_command(pointer),
        }
    }

    /// Pascal `DoFileEditCmd` (`ExecHelper.pas:1681`): an existing file goes to
    /// `FireOffEditor` — the GUI editor launch, a headless no-op (the
    /// established `AllowEditor` convention); a missing file sets
    /// `GlobalResult` (no error). The path resolves against the engine's
    /// virtual cwd like every file argument.
    fn do_file_edit_cmd(&mut self) {
        self.parser.next_param(&self.vars);
        let param = self.parser.make_string(&self.vars).to_string();
        if !self.current_dir.join(&param).is_file() {
            self.last_result = format!("File \"{param}\" does not exist.");
        }
    }

    /// Pascal `ord(Cmd.CD)` (`ExecCommands.pas:315`): change the data path to
    /// an EXISTING directory (`SetDataPath`; unlike `Set DataPath=` it never
    /// creates one) — error #282 on a miss. Like the shared `Set DataPath=`
    /// port (`set_cmd.rs::apply_data_path`), the Pascal non-writable-dir →
    /// scratch `OutputDirectory` fallback (`DSSGlobals.pas:562-568`) is
    /// NOT_PORTED — an environment-dependent I/O rescue, not oracle-pinnable;
    /// a later write fails loudly instead (audit WP8.8: consistent recorded
    /// narrowing, corpus-unreachable).
    fn do_cd_cmd(&mut self) {
        self.parser.next_param(&self.vars);
        let param = self.parser.make_string(&self.vars).to_string();
        let p = self.current_dir.join(&param);
        if p.is_dir() {
            self.current_dir = p.clone();
            self.output_directory = p;
        } else {
            self.errors
                .push(format!("Directory \"{param}\" not found."));
        }
    }

    /// Pascal `DoVarCmd` (`ExecHelper.pas:4889`): the `var` script-variable
    /// command — bare `var` lists every parser variable (`Variable, Value`
    /// header + one `<name>. <value|null>` line each), `var @x` echoes the
    /// substituted value, `var @x=1 @y=2` defines/overwrites variables (a name
    /// not starting with `@` is error #28725 and stops the scan).
    fn do_var_cmd(&mut self) {
        let mut param_name = self.parser.next_param(&self.vars);
        let mut param = self.parser.make_string(&self.vars).to_string();
        if param.is_empty() {
            // Show all vars.
            let mut s = String::from("Variable, Value\n");
            for i in 0..self.vars.len() {
                s.push_str(&self.vars.var_string(i));
                s.push('\n');
            }
            self.last_result = s;
        } else if param_name.is_empty() {
            // Show this var's value (the parser already substituted it).
            self.last_result = param;
        } else {
            while !param_name.is_empty() {
                if !param_name.starts_with('@') {
                    self.errors.push(format!(
                        "Illegal Variable Name: {param_name}; Must begin with \"@\""
                    ));
                    return;
                }
                self.vars.add(&param_name, &param);
                param_name = self.parser.next_param(&self.vars);
                param = self.parser.make_string(&self.vars).to_string();
            }
        }
    }

    fn not_ported_command(&mut self, pointer: usize) {
        let name = EXEC_COMMANDS.get(pointer - 1).copied().unwrap_or("?");
        self.errors
            .push(format!("Command \"{name}\" is not ported yet."));
    }

    /// Pascal `ExecCommands.pas` `ord(Cmd.CalcIncMatrix)` / `CalcIncMatrix_O`
    /// (`Solution.Calc_Inc_Matrix` / `Calc_Inc_Matrix_Org`): build the
    /// branch-to-node incidence matrix, flat or hierarchically organized (WP-AD.1).
    fn do_calc_inc_matrix(&mut self, organized: bool) {
        let Some(ckt) = self.circuit.as_mut() else {
            return;
        };
        if organized {
            crate::solution::inc_matrix::calc_inc_matrix_org(&mut self.classes, ckt);
        } else {
            crate::solution::inc_matrix::calc_inc_matrix(&self.classes, ckt);
        }
    }

    /// Pascal `ExecCommands.pas` cmd 114 (`Refine_BusLevels`): run
    /// `Get_paths_4_Coverage` (trace the longest paths from the feeder backbone up
    /// to the requested `Coverage`), then `GlobalResult := IntToStr(
    /// length(Path_Idx)-1) + ' new paths detected'` (ExecCommands.pas:691–694).
    /// Requires a prior `CalcIncMatrix_O` (the hierarchical levels); with no
    /// incidence matrix the coverage tracer reports 0 new paths (WP-AD.5).
    fn do_refine_bus_levels(&mut self) {
        let Some(ckt) = self.circuit.as_mut() else {
            return;
        };
        ckt.get_paths_4_coverage();
        let new_paths = ckt.ad.path_idx.len().saturating_sub(1);
        self.last_result = format!("{new_paths} new paths detected");
    }

    /// Pascal `ExecCommands.pas` `ord(Cmd.CalcLaplacian)`: `Laplacian :=
    /// IncMat.Transpose(); Laplacian := Laplacian.multiply(IncMat)`. The NIL guard
    /// (error 8877) fires when no incidence matrix has been calculated yet — the
    /// message text (including the upstream "Indidence" typo) is verbatim.
    fn do_calc_laplacian(&mut self) {
        let Some(ckt) = self.circuit.as_mut() else {
            return;
        };
        let st = &mut ckt.solution.inc_matrix;
        let Some(inc_mat) = st.inc_mat.as_ref() else {
            self.errors.push(crate::diag::DssDiagnostic::msg(
                "Indidence matrix is not present. Please run either \"CalcIncMatrix\" or \"CalcIncMatrix_O\" first.",
                Some(8877),
            ));
            return;
        };
        let laplacian = inc_mat.transpose().multiply(inc_mat);
        st.laplacian = Some(laplacian);
    }

    /// Pascal `GetObjClassAndName`: read the `class.name` token (optionally
    /// prefixed `object=`) from the main parser.
    pub(super) fn get_obj_class_and_name(&mut self) -> (String, String) {
        let param_name = self.parser.next_param(&self.vars).to_ascii_lowercase();
        let param = self.parser.make_string(&self.vars);
        if !param_name.is_empty() && !crate::util::compare_text_shortest_eq(&param_name, "object") {
            // Pascal error 240 (ExecHelper.pas:219): the `%s` argument is
            // `CRLF + Parser.CmdString` (`sLineBreak`, rendered LF here like
            // every other output line).
            self.errors.push(crate::diag::DssDiagnostic::msg(
                format!(
                    "object=Class.Name expected as first parameter in command. \n{}",
                    self.parser.cmd_string()
                ),
                Some(240),
            ));
            return (String::new(), String::new());
        }
        parse_object_class_and_name(&mut self.parser, &self.vars, &param)
    }

    /// Pascal `DSSClassDefs.SetObjectClass`: activate the class named `param`.
    /// Sets `active_class` — the unified field for both Pascal
    /// `LastClassReferenced` AND `ActiveDSSClass`. The r3875 fix
    /// (dss_capi `7457fc0b`, C11) added the `ActiveDSSClass := …` assignment so
    /// SetObjectClass *always* activates the selected class; because the port
    /// collapses the two Pascal fields into one, that fix is inherent here. An
    /// unknown class logs #903 and leaves the previously-referenced class in
    /// place. WP-U1.1 item 5.
    pub(super) fn set_object_class(&mut self, param: &str) {
        match self.class_by_name.get(&param.to_ascii_lowercase()).copied() {
            Some(ci) => self.active_class = Some(ci),
            None => self.errors.push(crate::diag::DssDiagnostic::msg(
                format!(
                    "Error! Object Class \"{param}\" not found. \n{}",
                    self.parser.cmd_string()
                ),
                Some(903),
            )),
        }
    }

    /// Pascal `DSSGlobals.SetObject`: set the active object by `class.name`
    /// (or bare `name` against the active class).
    pub(super) fn set_object(&mut self, param: &str) -> bool {
        let (class_part, name_part) = match param.find('.') {
            Some(p) => (param[..p].to_string(), param[p + 1..].to_string()),
            None => (String::new(), param.to_string()),
        };
        // Pascal `if Length(ObjClass) > 0 then SetObjectClass(ObjClass)`: a class
        // qualifier activates that class; an UNKNOWN one logs #903 and its FALSE
        // return is DISCARDED — the previously-referenced class stays active and the
        // name below is resolved against IT (fall-back), exactly like `do_select_cmd`.
        // Probed 2026-07-12: capi015 & 0.14.5 both keep #903 yet still select the
        // name in the previous class (`Set Object=badclass.l1` → `Line.l1`).
        if !class_part.is_empty() {
            self.set_object_class(&class_part);
        }
        // Pascal `ActiveDSSClass := Get(LastClassReferenced)` = our `active_class`;
        // NIL (no class ever referenced) → #905 "Active object type/class is not set."
        let Some(ci) = self.active_class else {
            self.errors.push(crate::diag::DssDiagnostic::msg(
                "Error! Active object type/class is not set.",
                Some(905),
            ));
            return false;
        };
        if !self.classes[ci].set_active(&name_part) {
            // Pascal #904: message uses the bare ObjName + the command string.
            self.errors.push(crate::diag::DssDiagnostic::msg(
                format!(
                    "Error! Object \"{name_part}\" not found. \n{}",
                    self.parser.cmd_string()
                ),
                Some(904),
            ));
            return false;
        }
        // Pascal `SetActive` also makes a circuit element the `ActiveCktElement`
        // (a general DSS_OBJECT does not) — so `Set Object=line.l1` followed by a
        // bare `? prop`/`~ prop=` reaches it. Mirrors `do_select_cmd`.
        let idx = self.classes[ci]
            .active
            .expect("set_active set the active index");
        if self.classes[ci].arena.try_ckt_elem(idx).is_some() {
            self.active_ckt_element = Some((ci, idx));
        }
        true
    }

    /// Pascal `DoNewCmd` → `MakeNewCircuit` / `AddObject`.
    fn do_new_cmd(&mut self) {
        let (obj_class, obj_name) = self.get_obj_class_and_name();
        if obj_class.eq_ignore_ascii_case("solution") {
            self.errors.push(
                "You cannot create new Solution objects through the command interface.".to_string(),
            );
            return;
        }
        if obj_class.eq_ignore_ascii_case("circuit") {
            self.make_new_circuit(&obj_name);
            return;
        }
        self.add_object(&obj_class, &obj_name);
    }

    /// Pascal `DSSGlobals.MakeNewCircuit`: create the circuit, then feed the
    /// remainder of the line to the default source —
    /// `New object=vsource.source Bus1=SourceBus <remainder>`.
    fn make_new_circuit(&mut self, name: &str) {
        if self.circuit.is_some() {
            // Pascal error 906; MaxCircuits is 1 in this build.
            self.errors.push(
                "MakeNewCircuit: Cannot create new circuit. Max. Circuits Exceeded. (Max no. of circuits=1)"
                    .to_string(),
            );
            return;
        }
        let mut ckt = Circuit::new(name, self.default_base_freq);
        // Pascal `TDSSCircuit.Create`: both default shape refs resolve to the
        // built-in `loadshape.default` (created with the default DSS items).
        ckt.default_daily_shape_obj = find_load_shape(&self.classes, "default");
        ckt.default_yearly_shape_obj = find_load_shape(&self.classes, "default");
        self.circuit = Some(ckt);
        let s = self.parser.remainder().to_string();
        self.command(&format!("New object=vsource.source Bus1=SourceBus {s}"));
    }

    /// Pascal `DoEditCmd` → `EditObject`.
    fn do_edit_cmd(&mut self) {
        let (obj_class, obj_name) = self.get_obj_class_and_name();
        if obj_class.eq_ignore_ascii_case("circuit") {
            return; // Do nothing if editing Circuit
        }
        let Some(&ci) = self.class_by_name.get(&obj_class.to_ascii_lowercase()) else {
            self.errors.push(format!(
                "Edit Command: Object Type \"{obj_class}\" not found."
            ));
            return;
        };
        self.active_class = Some(ci);
        if self.classes[ci].set_active(&obj_name) {
            self.edit_active();
        }
    }

    // `DoBatchEditCmd` (incl. the r4133 `where` conditionals + `Elements edited:
    // N` result) lives in `exec/batchedit.rs`.

    /// Pascal `DoEnableCmd`/`DoDisableCmd` (`ExecHelper.pas:1095/1145`):
    /// `Enable`/`Disable class[.name|.*]`. `circuit` → no-op; an unknown class
    /// or a non-circuit-element class (the `BASECLASSMASK` guard) → silently
    /// nothing (no error upstream); `*` → set `Enabled` directly on every
    /// element of the class (the bare `Set_Enabled` setter — NOT the edit path,
    /// so `PrpSequence`/`RecalcElementData` are untouched); a name → reload the
    /// parser with `Enabled=true|false` and run the ordinary edit
    /// (`EditObject`; a missing name is silently ignored, `SetActive` = false).
    fn do_enable_disable_cmd(&mut self, enable: bool) {
        let (obj_type, obj_name) = self.get_obj_class_and_name();
        if obj_type.is_empty() || obj_type.eq_ignore_ascii_case("circuit") {
            return; // Pascal: do nothing
        }
        let Some(&ci) = self.class_by_name.get(&obj_type.to_ascii_lowercase()) else {
            return; // Pascal: GetDSSClassPtr = NIL → nothing
        };
        if self.classes[ci].kind.is_none() {
            return; // Pascal: (DSSClassType and BASECLASSMASK) = 0 → nothing
        }
        if obj_name == "*" {
            let mut any_changed = false;
            for oi in 0..self.classes[ci].arena.len() {
                if let Some(elem) = self.classes[ci].arena.try_ckt_elem_mut(oi) {
                    let cd = elem.cd_mut();
                    cd.set_enabled(enable);
                    if cd.signal_bus_name_redefined {
                        cd.signal_bus_name_redefined = false;
                        any_changed = true;
                    }
                }
            }
            // Pascal `Set_Enabled` writes `BusNameRedefined` on the circuit
            // immediately; the signal-flag propagation is drained here since
            // this path bypasses the edit tail.
            if any_changed && let Some(ckt) = self.circuit.as_mut() {
                ckt.set_bus_name_redefined(true);
            }
        } else {
            self.active_class = Some(ci); // DSS.LastClassReferenced
            if !self.classes[ci].set_active(&obj_name) {
                return; // Pascal EditObject: SetActive false → nothing
            }
            self.parser.set_cmd_string(if enable {
                "Enabled=true"
            } else {
                "Enabled=false"
            });
            self.edit_active();
        }
    }

    /// Pascal `DoOpenCmd`/`DoCloseCmd` (ExecHelper.pas:1451/1484): open
    /// (`close=false`) or close (`close=true`) a terminal — and optionally a
    /// single conductor — of a circuit element.
    ///
    /// Syntax: `Open class.name term=xx cond=xx`. A `cond` of 0 (omitted) opens
    /// the **whole** terminal (`Closed[0]`); `cond>0` opens just that 1-based
    /// conductor. `Closed[...] := …` raises `YPrimInvalid` →
    /// `SystemYChanged` unconditionally (the step-2a dirty-edge rule), so the
    /// next `BuildYMatrix` rebuilds with the conductor open. Editing `circuit` is
    /// a no-op, exactly as Pascal.
    ///
    /// Pascal additionally calls `SetActiveBus(StripExtension(Getbus(...)))` to
    /// make the switched terminal's bus active; no ported command consumes an
    /// active bus, so that inert side effect is not reproduced (revisit when the
    /// bus-context verbs land).
    fn do_open_close_cmd(&mut self, close: bool) {
        let verb = if close { "Close" } else { "Open" };
        // Pascal `SetActiveCktElement` resolves the leading `class.name` to an
        // active circuit element (None on any failure — unknown class / object /
        // a non-element / the `circuit` object). Its own diagnostics (253/254)
        // are emitted there; the outer DoOpenCmd/DoCloseCmd adds 259/260 below.
        let resolved = self.set_active_ckt_element(verb);
        let Some(ci) = resolved else {
            self.errors.push(format!(
                "Error in {verb} Command: Circuit Element not found."
            ));
            return;
        };
        // `term` then `cond` (Pascal `NextParam; IntValue` — empty => 0).
        self.parser.next_param(&self.vars);
        let terminal = self.parser.make_integer(&self.vars).unwrap_or(0).max(0) as usize;
        self.parser.next_param(&self.vars);
        let conductor = self.parser.make_integer(&self.vars).unwrap_or(0).max(0) as usize;

        let idx = self.classes[ci]
            .active
            .expect("set_active set the active index");
        let dirty = {
            let cd = self.classes[ci]
                .arena
                .try_ckt_elem_mut(idx)
                .expect("set_active_ckt_element returned a circuit element")
                .cd_mut();
            // Pascal `ActiveTerminalIdx := Terminal; Closed[Conductor] := …`.
            // `Set_ActiveTerminal` (CktElement.pas:253) only adopts a terminal in
            // `1..Nterms`; an omitted/out-of-range `term=` leaves the active
            // terminal unchanged (default = terminal 1, `FActiveTerminal = 0`).
            // `Closed[Conductor]` (cond 0 => all phases) then acts on that *active*
            // terminal — so `Open class.name` with no `term=` opens terminal 1; it
            // is NOT a no-op. (Each `Closed[…]` raises YPrimInvalid.)
            if terminal >= 1 && terminal <= cd.nterms {
                cd.active_terminal = terminal - 1;
            }
            let active = cd.active_terminal + 1; // 1-based for the setters
            if conductor == 0 {
                cd.set_terminal_closed(active, close);
            } else {
                cd.set_conductor_closed(active, conductor, close);
            }
            cd.yprim_invalid && cd.enabled
        };
        // …which propagates to SystemYChanged unconditionally (the step-2a
        // dirty-edge rule), so the next BuildYMatrix rebuilds with the change.
        if dirty && let Some(ckt) = self.circuit.as_mut() {
            ckt.solution.system_y_changed = true;
        }
    }

    /// Pascal `TExecHelper.SetActiveCktElement`: read the leading `class.name`,
    /// set it active, and return its class index iff it is a real circuit
    /// element. Returns `None` (with the matching Pascal diagnostic) for the
    /// `circuit` object, an unknown class (253), an unknown object, or a
    /// non-circuit object (254).
    fn set_active_ckt_element(&mut self, verb: &str) -> Option<usize> {
        let (obj_class, obj_name) = self.get_obj_class_and_name();
        if obj_class.eq_ignore_ascii_case("circuit") {
            return None; // Pascal: do nothing (retval stays 0)
        }
        let Some(&ci) = self.class_by_name.get(&obj_class.to_ascii_lowercase()) else {
            self.errors.push(format!(
                "Error in {verb} Command: Object Type \"{obj_class}\" not found."
            ));
            return None;
        };
        self.active_class = Some(ci);
        if !self.classes[ci].set_active(&obj_name) {
            return None;
        }
        let idx = self.classes[ci]
            .active
            .expect("set_active set the active index");
        if self.classes[ci].arena.try_ckt_elem(idx).is_none() {
            self.errors.push(format!(
                "Error in {verb}: Object not a circuit Element. {obj_class}.{obj_name}"
            ));
            return None;
        }
        Some(ci)
    }

    /// Pascal `TExecHelper.DoSelectCmd` (`ExecHelper.pas:670`): make a circuit
    /// element (or the active object) active — `Select class.name [terminal]`. Sets
    /// `ActiveCktElement` (read by `Show Yprim`) and the element's active terminal.
    ///
    /// A bare `Select` (no class/name) selects the already-active object (a no-op
    /// here), and `Select circuit` switches the active circuit (single-circuit
    /// build → no-op). Bus-context (`SetActiveBus`) is not reproduced — no ported
    /// report consumes an active bus yet (same inert side effect skipped by
    /// [`Dss::do_open_close_cmd`]).
    fn do_select_cmd(&mut self) {
        let (obj_class, obj_name) = self.get_obj_class_and_name();
        if obj_class.is_empty() && obj_name.is_empty() {
            return; // "select active obj if any"
        }
        if obj_class.eq_ignore_ascii_case("circuit") {
            return; // SetActiveCircuit — single circuit, nothing to switch
        }
        // Pascal `if Length(ObjClass)>0 then SetObjectClass(ObjClass)`: a known
        // class becomes the active/"last-referenced" class; an UNKNOWN one logs
        // #903 and leaves the previously-referenced class in place (the select then
        // falls back to it) — it does NOT abort. An empty class keeps the previous
        // class unchanged.
        if !obj_class.is_empty() {
            match self
                .class_by_name
                .get(&obj_class.to_ascii_lowercase())
                .copied()
            {
                Some(ci) => self.active_class = Some(ci),
                None => self
                    .errors
                    .push(format!("Error! Object Class \"{obj_class}\" not found. ")),
            }
        }
        // Pascal `ActiveDSSClass := Get(LastClassReferenced)` = our `active_class`;
        // `NIL` (no class ever referenced) → #246.
        let Some(ci) = self.active_class else {
            self.errors.push(crate::diag::DssDiagnostic::msg(
                "Error! Active object type/class is not set.",
                Some(246),
            ));
            return;
        };
        if !self.classes[ci].set_active(&obj_name) {
            // Pascal #245.
            self.errors.push(crate::diag::DssDiagnostic::msg(
                format!("Error! Object \"{obj_name}\" not found. "),
                Some(245),
            ));
            return;
        }
        let idx = self.classes[ci]
            .active
            .expect("set_active set the active index");
        // Only circuit elements become the `ActiveCktElement` (Pascal: a general
        // `DSS_OBJECT` does nothing here).
        if self.classes[ci].arena.try_ckt_elem(idx).is_some() {
            self.active_ckt_element = Some((ci, idx));
            // Active terminal (Pascal `if Length(Param)>0 then ActiveTerminalIdx :=
            // IntValue else 1`): an **absent** param selects terminal 1; a
            // **present** one is adopted only when in `1..Nterms`
            // (`Set_ActiveTerminal`), else the active terminal is left unchanged.
            self.parser.next_param(&self.vars);
            let param = self.parser.make_string(&self.vars).to_string();
            let tval = if param.is_empty() {
                None
            } else {
                Some(self.parser.make_integer(&self.vars).unwrap_or(0))
            };
            let cd = self.classes[ci]
                .arena
                .try_ckt_elem_mut(idx)
                .expect("just checked it is a circuit element")
                .cd_mut();
            match tval {
                None if cd.nterms >= 1 => cd.active_terminal = 0,
                None => {}
                Some(t) if t >= 1 && (t as usize) <= cd.nterms => {
                    cd.active_terminal = (t - 1) as usize
                }
                Some(_) => {}
            }
        }
    }

    /// Pascal `DoVisualizeCmd` (`ExecHelper.pas:4099-4197`). The plot itself
    /// goes to `DSSPlotCallback` — NIL in the pinned headless oracle — so past
    /// the guards this is a faithful no-op (PHASE8_PLAN §2.5). The guards run
    /// BEFORE the callback check and ARE engine-observable, so they are
    /// ported: #24722 on an unsolved circuit (`Solution.NodeV` unallocated),
    /// #282 element-not-found. Notes:
    /// - the no-circuit #24721 arm is unreachable here — the dispatcher's
    ///   generic pre-circuit guard already emits #301 (oracle-probed, WP8.1);
    /// - the `"%s" must be a circuit element type!` #282 arm is dead upstream:
    ///   `GetCktElementIndex` (`Utilities.pas:733`) resolves through
    ///   `element.Handle`, and a general (non-circuit) `DSSObject`'s Handle is
    ///   0 → the not-found arm fires instead, so a `loadshape.x` reference
    ///   lands on "not found" — reproduced by resolving through circuit-element
    ///   classes only.
    fn do_visualize_cmd(&mut self) {
        // `not assigned(Solution.NodeV)` → #24722. `node_v` starts as the
        // ground-only slot `[0]` and is sized by the first Y build/solve
        // (`ymatrix::build_y_matrix`, `allocate_vi`).
        if self
            .circuit
            .as_ref()
            .is_none_or(|c| c.solution.node_v.len() <= 1)
        {
            self.errors
                .push("The circuit must be solved before you can do this.".to_string());
            return;
        }

        // Parse `What=`/`Element=` (`CompareTextShortest` prefixes; bare
        // values fill positions 1, 2, …; unknown names are skipped). `What`
        // (the plotted quantity) is carried into the callback JSON's `Quantity`;
        // it has no engine-observable effect otherwise. Default `Current`
        // (`ExecHelper.pas:4124`).
        let mut elem_name = String::new();
        let mut quantity = "Current";
        let mut pointer = 0usize;
        loop {
            let param_name = self.parser.next_param(&self.vars);
            let param = self.parser.make_string(&self.vars);
            if param.is_empty() {
                break;
            }
            if param_name.is_empty() {
                pointer += 1;
            } else if crate::util::compare_text_shortest_eq(&param_name, "WHAT") {
                pointer = 1;
            } else if crate::util::compare_text_shortest_eq(&param_name, "ELEMENT") {
                pointer = 2;
            } else {
                continue; // Unknown named parm — ignored (Pascal `Unknown`).
            }
            match pointer {
                1 => {
                    // First letter of the value → the plotted quantity
                    // (`ExecHelper.pas:4148`).
                    quantity = match param.as_bytes().first().map(u8::to_ascii_lowercase) {
                        Some(b'c') => "Current",
                        Some(b'v') => "Voltage",
                        Some(b'p') => "Power",
                        _ => quantity,
                    };
                }
                2 => elem_name = param.to_string(),
                _ => {}
            }
        }

        // `GetCktElementIndex` (`Utilities.pas:733`): split `Class.Name` at
        // the first dot; an unknown/absent class falls back to the
        // last-referenced class; an empty name or a general-object class
        // (Handle = 0, see the doc note) → not found.
        let (cls_str, obj_str) =
            crate::util::parse_object_class_and_name(&mut self.parser, &self.vars, &elem_name);
        let ci = match self.class_by_name.get(&cls_str.to_ascii_lowercase()) {
            Some(&ci) => Some(ci),
            None => self.active_class, // `DSS.LastClassReferenced` fallback
        };
        let found = ci.is_some_and(|ci| {
            self.classes[ci].kind.is_some()
                && !obj_str.is_empty()
                && self.classes[ci]
                    .name_to_idx
                    .contains_key(&obj_str.to_ascii_lowercase())
        });
        if !found {
            self.errors.push(format!(
                "Requested Circuit Element: \"{elem_name}\" not found."
            ));
            return;
        }
        // Found: build the Visualize JSON `{PlotType, ElementName, ElementType,
        // Quantity}` (`ExecHelper.pas:4184`) and fire the callback. `ElementType`
        // is the element's DSS class name; `ElementName` its Name. With no
        // callback registered this is a faithful no-op (byte-identical to the
        // pinned headless oracle, which runs `DSSPlotCallback = NIL`).
        let ci = ci.expect("found implies ci is Some");
        let oi = self.classes[ci]
            .name_to_idx
            .get(&obj_str.to_ascii_lowercase())
            .copied()
            .expect("found confirmed the object exists");
        let element_name = self.classes[ci].arena.obj(oi).data().name().to_string();
        let element_type = self.classes[ci].props.class_name().to_string();
        self.fire_visualize_callback(&element_name, &element_type, quantity);
    }

    /// Pascal `AddObject`: create the object (or make the existing one
    /// active for `DSS_OBJECT` classes), register circuit elements with the
    /// circuit, and edit the rest of the line.
    pub(super) fn add_object(&mut self, obj_class: &str, name: &str) {
        if self.create_object_no_edit(obj_class, name) {
            self.edit_active();
        }
    }

    /// The object-creation half of [`Dss::add_object`] (Pascal `AddObject` up to
    /// but not including the `TDSSClass.Edit` of the remaining parameters):
    /// resolve/validate the class, create the object (or activate the existing
    /// `DSS_OBJECT`), apply the per-class creation defaults and register a
    /// circuit element with the circuit. Returns `true` when an object is active
    /// and ready to be edited (so `add_object` runs `edit_active`); the JSON
    /// reader instead applies `FillObjFromJSON`. This split matches the Pascal
    /// `obj_NewFromClass` (create, no `RecalcElementData`) used by
    /// `loadClassFromJSON` — a JSON-imported element that needs another element
    /// (RegControl → its transformer) must not run its recalc on the empty
    /// pre-fill object.
    pub(super) fn create_object_no_edit(&mut self, obj_class: &str, name: &str) -> bool {
        let Some(&ci) = self.class_by_name.get(&obj_class.to_ascii_lowercase()) else {
            self.errors.push(format!(
                "New Command: Object Type \"{obj_class}\" not found."
            ));
            return false;
        };
        self.active_class = Some(ci);

        if name.is_empty() {
            self.errors.push("Object Name Missing".to_string());
            return false;
        }

        if self.classes[ci].requires_circuit && self.circuit.is_none() {
            self.errors
                .push("You Must Create a circuit first: \"new circuit.yourcktname\"".to_string());
            return false;
        }

        if !self.classes[ci].requires_circuit {
            // Pascal `TTCC_Curve.NewObject` (SVN r4119, fd034bb0): `none` is a
            // reserved TCC_Curve name — it means "no curve" when referenced by a
            // circuit element — so creating one errors (423) and NewObject returns
            // NIL, which the caller's `if obj=NIL then Exit` guard drops (here: we
            // return without adding). WP-U1.1 item 4.
            if name.eq_ignore_ascii_case("none")
                && self.classes[ci]
                    .props
                    .class_name()
                    .eq_ignore_ascii_case("TCC_Curve")
            {
                self.errors.push(
                    "TCC_Curve: \"NONE\", \"none\" is a reserved name that means no curve \
                     specified when referenced by circuit elements. A different name must be \
                     specified. Error in definition of object."
                        .to_string(),
                );
                return false;
            }
            // DSS_OBJECT path: duplicates become edits.
            if !self.classes[ci].set_active(name) {
                let idx = self.classes[ci].arena.push_new(name);
                let obj_name = self.classes[ci].arena.obj(idx).data().name().to_string();
                self.classes[ci].name_to_idx.insert(obj_name, idx);
                self.classes[ci].active = Some(idx);
                // Pascal `TLineCodeObj.Create`: `BaseFrequency :=
                // ActiveCircuit.Fundamental` (LineCode.pas:493). The LineCode is
                // the one DSS_OBJECT carrying a base frequency; it inherits the
                // circuit fundamental so a 50 Hz feeder's charging admittance is
                // computed at 50 Hz (propagated to lines via `FetchLineCode`).
                // `edit_active`'s `EndEdit` recomputes the matrices at this freq.
                if let Some(fund) = self.circuit.as_ref().map(|c| c.fundamental)
                    && let Some(lc) = self.classes[ci]
                        .arena
                        .get_mut::<line_code::LineCodeObj>(idx)
                {
                    lc.set_base_frequency(fund);
                }
                // Pascal `DSS.DSSObjs.Add(Obj)` (`ExecHelper.pas:1899`): the
                // global creation-order list the whole-circuit Dump walks.
                self.dss_objs.push(ElemId::new(ci, idx));
            }
            return true;
        }

        // Circuit-element path. Duplicate names: warn and bail (the Pascal
        // exits without editing when DuplicatesAllowed is off).
        let duplicates_allowed = self.circuit.as_ref().is_some_and(|c| c.duplicates_allowed);
        if !duplicates_allowed && self.classes[ci].set_active(name) {
            self.errors.push(format!(
                "Warning: Duplicate new element definition: \"{}.{}\". Element being redefined.",
                self.classes[ci].props.class_name(),
                name
            ));
            return false;
        }

        let idx = self.classes[ci].arena.push_new(name);
        let obj_name = self.classes[ci].arena.obj(idx).data().name().to_string();
        self.classes[ci].name_to_idx.insert(obj_name, idx);
        self.classes[ci].active = Some(idx);

        // Pascal `TDSSCktElement.Create`: `BaseFrequency := ActiveCircuit.Fundamental`
        // (CktElement.pas:203). Every circuit element inherits the circuit's base
        // frequency at creation, so `Set DefaultBaseFrequency=50` (before `New
        // circuit`) makes a European feeder run at 50 Hz. `edit_active` (below) can
        // still override via `basefreq=`.
        //
        // **Every** element, Monitor included — which upstream is not. There
        // `TMonitorObj.Create` re-hardcodes `Basefrequency := 60.0` AFTER the
        // inherited `Create` (`.inputs/dss_capi/src/Meters/Monitor.pas:472`;
        // r4133 `Version8/Source/Meters/Monitor.pas:552`), overriding the
        // fundamental every other element inherits — the sibling measurement
        // classes (EnergyMeter, Sensor) do not, and `Line.pas:974` /
        // `GICLine.pas:373` carry the very same assignment *commented out* with
        // "set in base class", so the Monitor's is left-over rather than meant.
        // It has one physical consumer, mode-4 flicker: the field is the `fBase`
        // handed to `FlickerMeter` (`Monitor.pas:1657` → `Pstcalc.pas:594`),
        // where `fBase = 50.0` selects the IEC 61000-4-15 230 V/50 Hz lamp
        // weighting instead of the 120 V/60 Hz set (`Pstcalc.pas:609-626`), so a
        // 50 Hz feeder's Pst comes out on the wrong lamp curve unless the user
        // writes `basefreq=50` by hand. `GOLDEN_REBASE_PLAN.md` G2.2b tore the
        // reproduction down: both lanes inherit, and the property compare that
        // observes it (`Monitor.BaseFreq` on the 50 Hz LVTestCase) is excluded in
        // both lanes and pinned by
        // `exec::tests::base_frequency::monitor_basefreq_inherits_the_fundamental`.
        let fundamental = self.circuit.as_ref().expect("checked above").fundamental;
        self.classes[ci]
            .arena
            .try_ckt_elem_mut(idx)
            .expect("circuit element class builds circuit elements")
            .cd_mut()
            .base_frequency = fundamental;

        // Pascal `TVsourceObj.Create`/`TIsourceObj.Create`: `SrcFrequency :=
        // BaseFrequency` (VSource.pas:644, Isource.pas:319) — the source frequency
        // defaults to the inherited base frequency, not a hardcoded 60 Hz. Without
        // this a 50 Hz feeder's VSource keeps SrcFrequency=60, so the frequency
        // mismatch check (`VSource.pas:1071`) zeroes Vmag and the whole feeder dies.
        // A later `frequency=` edit still wins (applied in `edit_active`).
        if let Some(vs) = self.classes[ci].arena.get_mut::<vsource::VSource>(idx) {
            vs.src_frequency = fundamental;
        } else if let Some(is) = self.classes[ci].arena.get_mut::<isource::Isource>(idx) {
            is.src_frequency = fundamental;
        }

        // Pascal `TLineObj.Create` copies the context default earth model into
        // `FEarthModel` (Line.pas:998); a later `EarthModel=` edit can override
        // it. Applied before `edit_active` so the property still wins.
        //
        // Pascal `TLineObj.Create` also ends with `RecalcElementData`
        // (Line.pas:1001), which reads the live `ActiveCircuit.PositiveSequence`
        // (Line.pas:1085). Sync the flag and, in a `CktModel=Positive` circuit,
        // re-run the collapse so a defaults-only `New Line` shows collapsed
        // r0/x0/c0 at create time (readback-observable; probe-proven).
        let positive_sequence = self
            .circuit
            .as_ref()
            .expect("checked above")
            .positive_sequence;
        if let Some(line) = self.classes[ci].arena.get_mut::<line::Line>(idx) {
            line.earth_model = self.default_earth_model;
            line.set_positive_sequence(positive_sequence);
            if positive_sequence {
                line.recalc_pos_seq();
            }
        }

        // Pascal `T<PC>Obj.Create` ends with `RecalcElementData`, which reads the
        // live `ActiveCircuit.Solution` globals (Mode / DynaVars / GenMultiplier /
        // LoadMultiplier / ActiveLoadShapeClass / PositiveSequence). Run that live
        // recalc now for the PC classes whose recalc consumes those globals, so a
        // property setter or `add_ckt_element` reading a recalc-derived field
        // during the edit block sees Pascal's Create-time value (`end_edit` re-runs
        // it after the edit). Consistent with the JSON "create, no recalc" split:
        // unlike RegControl, these recalcs read only own props + the live snapshot
        // (never another element).
        let sys = crate::solution::solution::sys_ctx(self.circuit.as_ref().expect("checked above"));
        recalc_pc_create(&mut self.classes[ci].arena, idx, &sys);

        let kind = self.classes[ci]
            .kind
            .expect("circuit element class has a kind");
        let ckt = self.circuit.as_mut().expect("checked above");
        let elem = self.classes[ci].arena.ckt_elem_mut(idx);
        ckt.add_ckt_element(ElemId::new(ci, idx), kind, elem);

        true
    }

    /// Pascal `DoClearCmd`: drop the circuit and every object. The executive
    /// first flushes any open demand-interval files (`Executive.pas:283`
    /// `Clear` → `if DIFilesAreOpen then CloseAllDIFiles`) so a yearly run's
    /// pending `DI_*` data is written, not lost with the circuit.
    fn do_clear_cmd(&mut self) {
        if self
            .circuit
            .as_ref()
            .is_some_and(|c| c.em_di.di_files_are_open)
        {
            self.do_close_di_cmd();
        }
        // Drop every element together (Pascal `Clear` resets the whole registry
        // — the `Idx<T>` stability invariant: arenas are only ever reset as a
        // set, never individually deleted).
        for cls in &mut self.classes {
            cls.arena.clear();
            cls.name_to_idx.clear();
            cls.active = None;
        }
        self.active_class = None;
        // `ActiveCktElement` is a field of `TDSSCircuit`; `Clear` destroys and
        // recreates the circuit, so it must reset to `None` too (else a stale
        // `(cls, idx)` from a pre-`Clear` `Select` indexes the now-emptied class
        // objects — an OOB panic / foreign-element read in `Show Yprim`).
        self.active_ckt_element = None;
        self.circuit = None;
        self.dss_objs.clear();
        self.errors.clear();
        // Pascal `DoClearCmd` → `ClearAll` → recreate the default items.
        self.create_default_dss_items();
    }

    /// Pascal `DoQueryCmd`: `? class.obj.prop` → store the property value in
    /// [`Dss::last_result`].
    fn do_query_cmd(&mut self) {
        self.parser.next_param(&self.vars);
        let full = self.parser.make_string(&self.vars);
        let (obj_name, prop_name) = parse_obj_name(&full);

        let (class_name, name) = {
            // Reuse the class.name splitter (no @var work needed on a query).
            let mut p = Parser::new();
            parse_object_class_and_name(&mut p, &self.vars, &obj_name)
        };

        self.last_result = "Property Unknown".to_string();
        let Some(&ci) = self.class_by_name.get(&class_name.to_ascii_lowercase()) else {
            self.errors
                .push(format!("Error! Object \"{obj_name}\" not found."));
            return;
        };
        self.active_class = Some(ci);
        if !self.classes[ci].set_active(&name) {
            self.errors
                .push(format!("Error! Object \"{obj_name}\" not found."));
            return;
        }
        let oi = self.classes[ci].active.expect("just set active");
        if let Some(idx) = self.classes[ci].props.property_index(&prop_name) {
            self.refresh_vterminal_if_marked(ci, oi, Some(idx));
            self.last_result =
                self.classes[ci]
                    .props
                    .get_value(self.classes[ci].arena.obj(oi), idx, &self.enums);
        }
    }

    /// Refresh a `&self` getter's live-state dependencies before a `?`/`Dump`
    /// property render, exactly for the properties that declare the need — the
    /// Rust choke point for readers that Pascal backs with a live pointer the
    /// `&self` getter can't reach across the class registry:
    ///
    /// 1. `cd.vterminal` from the solution when the property (`Some(idx)`) — or,
    ///    for the whole-object `Dump`, *any* property (`None`) — is marked
    ///    [`PropFlags::READS_VTERMINAL`]. Pascal's live-result getter reloads
    ///    `Vterminal` from `Solution.NodeV` itself
    ///    (`TTransfObj.GetAllWindingCurrents`, `Transformer.pas` l.1538).
    /// 2. A RegControl's `tap_snap` from its **live** controlled transformer
    ///    when reading `TapNum` (or the whole-object dump). Pascal `Get_TapNum`
    ///    reads `Transformer().PresentTap[TapWinding]` live; the Rust getter
    ///    reads a cached snapshot that a control action / direct `Taps=` edit
    ///    leaves stale (WP8.5b).
    /// 3. A StorageController's four fleet aggregates
    ///    (`kWhTotal`/`kWTotal`/`kWhActual`/`kWActual`, marked
    ///    [`PropFlags::RENDERS_LIVE_RESULT`]) from the live fleet. r4133's
    ///    getters re-sum `FleetPointerList` on every call
    ///    (`StorageController.pas:1162-1198`); the `&self` getter cannot reach
    ///    the Storage arena, so the sums land in the controller's render cache
    ///    here, immediately before the render (RP3.8).
    /// 4. An IndMach012's `pf` (same marker) from the present solution. r4133
    ///    renders `PowerFactor(Power[1, ActiveActor])` inside the getter
    ///    (`IndMach012.pas:1790`); the `&self` getter reaches no solution, so
    ///    the power factor lands in the machine's render cache here
    ///    ([`IndMach012::refresh_live_pf`], RP3.8).
    pub(super) fn refresh_vterminal_if_marked(
        &mut self,
        ci: usize,
        oi: usize,
        prop_idx: Option<usize>,
    ) {
        use crate::obj::props::PropFlags;
        let props = &self.classes[ci].props;
        let has_flag = |f: PropFlags| match prop_idx {
            Some(i) => props.prop(i).flags.contains(f),
            None => (1..=props.num_properties()).any(|i| props.prop(i).flags.contains(f)),
        };
        let marked = has_flag(PropFlags::READS_VTERMINAL);
        let renders_live_result = has_flag(PropFlags::RENDERS_LIVE_RESULT);
        if marked
            && let Some(node_v) = self.circuit.as_ref().map(|c| c.solution.node_v.clone())
            && let Some(elem) = self.classes[ci].arena.try_ckt_elem_mut(oi)
            // Pascal reloads Vterminal INSIDE the getter, after its
            // `if (not Enabled) or (NodeRef = NIL) or (NodeV = NIL) then Exit`
            // guard (e.g. `TTransfObj.GetAllWindingCurrents`, Transformer.pas:1530).
            // A DISABLED element is skipped by the re-solve's bus reprocessing, so
            // its `node_ref` stays stale (pointing at pre-conversion node numbers);
            // `MakePosSequence` disabling an off-phase-1 winding
            // (`makeposseq_xfmr.dss`) is the case that exposes it. The getter itself
            // already returns zeros for a disabled element, so mirror the guard here
            // and skip the (unsafe, stale-`node_ref`) refresh.
            && elem.cd().enabled
        {
            elem.cd_mut().compute_vterminal(&node_v);
        }

        // RegControl `TapNum` live-tap resync (see item 2 above). Gate on the
        // TapNum property (or the whole-object dump) so a non-RegControl class,
        // or a RegControl read of an unrelated property, does no registry walk.
        let touches_tapnum = match prop_idx {
            Some(i) => i == reg_control::prop::TAPNUM,
            None => true,
        };
        if touches_tapnum
            && let Some(tref) = self.classes[ci]
                .arena
                .get::<reg_control::RegControl>(oi)
                .and_then(|rc| rc.controlled_ref())
        {
            let rc_ref = ElemId::new(ci, oi);
            let mut store = ClassStore {
                classes: &mut self.classes,
            };
            // Either member of the Transformer/AutoTrans proxy.
            let (rc, tr_obj) =
                store.typed_transformer_pair_mut::<reg_control::RegControl>(rc_ref, tref);
            if let Some(tr) = tr_obj {
                rc.sync_tap_snap_from_live(tr);
            }
        }

        // Live-result properties (items 3-4): recompute the marked object's cached
        // live values from the store, so the `&self` getter renders the number
        // r4133's getter computes inside itself. Gated on the property actually
        // being marked (or a whole-object dump) — a class without such a
        // property does no registry walk.
        if renders_live_result {
            self.refresh_live_result_cache(ci, oi);
        }
    }

    /// The whole-store form of [`Self::refresh_vterminal_if_marked`], for the
    /// **`Save` serializer** (`Save circuit` and `Save <class>`).
    ///
    /// `?`, `Dump` and `element_properties` refresh one object at that choke
    /// point, gated on the property they are about to render. `Save` has no such
    /// gate: it walks whole classes and emits every property a deck explicitly
    /// **set**, in the order it set them (Pascal `GetNextPropertySet` ->
    /// `TDSSObject.SaveWrite`, `General/DSSObject.pas:145-165`) — and a write to
    /// a read-only property is silently ignored yet still marks the property
    /// set, so a deck line `New IndMach012.m1 ... pf=0.5` really does get a
    /// `PF=` back out of `Save`. r4133 fills those slots from the same live
    /// getters it uses for `?`, so the caches are refreshed once, up front, for
    /// every object; the per-object callee does the flag gating, so nothing but
    /// a marked property's cache moves (a pure read of the model).
    ///
    /// Measured through `epri-worker` on the r4133 DLL (2026-09-02), all three
    /// marked kinds — the port wrote the first column, r4133 the second:
    ///
    /// | saved property | before | r4133 |
    /// |---|---|---|
    /// | `IndMach012.PF` (deck wrote `pf=0.5`) | `1`, or the live value if a `?` came first | `0.886059` |
    /// | `StorageController.kWhTotal` (deck wrote `kWhTotal=42`) | `0` | `6000` |
    /// | `Transformer.WdgCurrents` (deck wrote `wdgcurrents=1`) | all-zero | the solved currents |
    ///
    /// The first row is why this exists: the pre-settlement answer depended on
    /// the session's **read history**, which is the contamination shape the
    /// corpus gate's own three-run artifact exists to forbid. The `WdgCurrents`
    /// row is the same latency on the older [`PropFlags::READS_VTERMINAL`]
    /// marker, pre-dating RP3.8 and fixed with it (RP3.8 audit settlement;
    /// no deck in the corpus or the goldens writes any of these properties, so
    /// no committed byte moves — swept 2026-09-02).
    ///
    /// `only = Some(ci)` restricts the pass to one class (`Save <class>`);
    /// `None` covers the store (`Save circuit`).
    pub(super) fn refresh_render_caches_for_save(&mut self, only: Option<usize>) {
        let classes: Vec<usize> = match only {
            Some(ci) => vec![ci],
            None => (0..self.classes.len()).collect(),
        };
        for ci in classes {
            for oi in 0..self.classes[ci].arena.len() {
                self.refresh_vterminal_if_marked(ci, oi, None);
            }
        }
    }

    /// The items-3-4 half of [`Self::refresh_vterminal_if_marked`]: recompute the
    /// object's live-result cache from the live store.
    ///
    /// StorageController — the four fleet aggregates
    /// (`StorageController.pas:991-994` -> `GetkWhTotal`/`GetkWTotal`/
    /// `GetkWhActual`/`GetkWActual`): collect each fleet member's live
    /// nameplate/state out of the Storage arena and hand them to
    /// [`StorageController::refresh_live_aggregates`], which sums them
    /// loop-for-loop. Nothing is written to the fleet, and the two
    /// `TotalkWhCapacity`/`TotalkWCapacity` fields r4133's `Var Sum` getters
    /// store into do not exist here (they are a dead store upstream — see
    /// `PropFlags::RENDERS_LIVE_RESULT`).
    ///
    /// IndMach012 — `pf` (`IndMach012.pas:1790`,
    /// `Format('%.6g', [PowerFactor(Power[1, ActiveActor])])`): hand the machine
    /// the live `SysCtx` + `NodeV` its `Get_Power` equivalent needs
    /// ([`IndMach012::refresh_live_pf`], which documents the fresh-`Iterminal`
    /// choice). Without a circuit there is no solution to read and the cache
    /// keeps its `PowerFactor(0) = 1` construction value, r4133's own no-power
    /// answer.
    fn refresh_live_result_cache(&mut self, ci: usize, oi: usize) {
        use crate::elements::control::storage_controller::{FleetMemberLive, StorageController};
        use crate::elements::pc::ind_mach012::IndMach012;

        if let Some(sc) = self.classes[ci].arena.get::<StorageController>(oi) {
            let fleet: Vec<ElemId> = sc.fleet_refs().to_vec();
            let store = ClassStore {
                classes: &mut self.classes,
            };
            let members: Vec<FleetMemberLive> = fleet
                .iter()
                .map(|&r| {
                    store
                        .typed::<storage::Storage>(r)
                        .expect("StorageController fleet entry is a Storage")
                })
                .map(|st| FleetMemberLive {
                    present_kw: st.present_kw(),
                    kwh_stored: st.kwh_stored,
                    kwh_rating: st.kwh_rating,
                    kw_rating: st.kw_rating,
                })
                .collect();
            if let Some(sc) = self.classes[ci].arena.get_mut::<StorageController>(oi) {
                sc.refresh_live_aggregates(&members);
            }
            return;
        }

        if self.classes[ci].arena.get::<IndMach012>(oi).is_some() {
            if let Some(ckt) = self.circuit.as_ref() {
                let sys = crate::solution::solution::sys_ctx(ckt);
                let node_v = ckt.solution.node_v.clone();
                if let Some(im) = self.classes[ci].arena.get_mut::<IndMach012>(oi) {
                    im.refresh_live_pf(&sys, &node_v);
                }
            }
            return;
        }

        // Reached only for a class that carries `RENDERS_LIVE_RESULT` (the
        // caller gates on it) and has no arm above — which would render a stale
        // cache silently, the very failure the flag exists to prevent. Debug
        // builds (every test run) say so; the release-build twin is the registry
        // walk `exec::tests::report::renders_live_result_holders_have_a_refresh_arm`.
        debug_assert!(
            false,
            "{} carries RENDERS_LIVE_RESULT but `refresh_live_result_cache` has \
             no arm for it — its render would serve whatever the cache last held",
            self.classes[ci].props.class_name()
        );
    }

    /// The body of Pascal `TDSSClass.Edit`: iterate `name=value` parameters on
    /// the main parser against the active object, then `EndEdit`, then
    /// propagate the element's signal flags to the circuit (the Pascal set
    /// `ActiveCircuit.BusNameRedefined`/`Solution.SystemYChanged` directly
    /// from the property setters; nothing reads them mid-edit, so polling
    /// after the edit is equivalent).
    pub(super) fn edit_active(&mut self) {
        self.edit_active_inner();

        // Pascal `TStorageControllerObj.RecalcElementData` ends every edit
        // line by building the fleet and pushing the controller's rates onto
        // it (see `storage_controller_recalc_fleet`); that needs the whole
        // store, so it runs here, after the split-borrow edit re-assembles
        // `self`.
        let Some(ci) = self.active_class else { return };
        let Some(oi) = self.classes[ci].active else {
            return;
        };
        if !self.classes[ci]
            .arena
            .get::<crate::elements::control::storage_controller::StorageController>(oi)
            .is_some()
            || self.circuit.is_none()
        {
            return;
        }
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("checked above");
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        crate::solution::controls::storage_controller_recalc_fleet(
            ElemId::new(ci, oi),
            ckt,
            &mut env,
        );
    }

    fn edit_active_inner(&mut self) {
        let Some(ci) = self.active_class else {
            self.errors
                .push("There is no active element to edit.".to_string());
            return;
        };
        let Dss {
            classes,
            circuit,
            parser,
            aux_parser,
            vars,
            enums,
            errors,
            current_dir,
            output_directory,
            last_result,
            last_result_file,
            cmd_origin,
            ..
        } = self;
        // Split the registry so the active class is borrowed mutably for the
        // edit while every *other* class is a read view for ObjectRef
        // resolution (PHASE4_PLAN §3.1). `split_at_mut` + `split_first_mut`
        // keep the three regions provably disjoint with no unsafe. The active
        // class's objects (`active_arena`) come with it (R1: objects live in
        // `DssClass::arena`), so no separate arena split is needed.
        let (left, rest) = classes.split_at_mut(ci);
        let (active_class, right) = rest.split_first_mut().expect("ci is in range");
        let foreign = ForeignClasses {
            left,
            right,
            split: ci,
        };
        let DssClass {
            props,
            arena: active_arena,
            name_to_idx,
            active,
            ..
        } = active_class;
        let Some(oi) = *active else {
            errors.push("There is no active element to edit.".to_string());
            return;
        };

        // Sync the live circuit context Pascal reads as globals inside
        // `RecalcElementData` / `CalcY_Terminal` (the pos-seq edit-time collapse
        // and the GIC `< 0.51 Hz` gate), so the side-effect recalcs run in this
        // edit see the current flag/frequency exactly as upstream. `New` also
        // routes here (create → edit_active), covering both parse and edit paths.
        let live_positive_sequence = circuit.as_ref().is_some_and(|c| c.positive_sequence);
        let live_frequency = circuit.as_ref().map_or(60.0, |c| c.solution.frequency);
        if let Some(line) = active_arena.get_mut::<line::Line>(oi) {
            line.set_positive_sequence(live_positive_sequence);
        } else if let Some(t) = active_arena.get_mut::<transformer::Transformer>(oi) {
            t.set_live_frequency(live_frequency);
        } else if let Some(a) = active_arena.get_mut::<auto_trans::AutoTrans>(oi) {
            a.set_live_frequency(live_frequency);
        }

        // Pascal `TDSSClass.BeginEdit` (`DSSClass.pas:1598`): any edit clears the
        // `DefaultAndUnedited` flag, so an edited default object rejoins the
        // whole-circuit JSON dump. Harmless on the initial `New` of the default
        // items themselves (the flag is set afterwards by `CreateDefaultDSSItems`).
        active_arena[oi].data_mut().set_default_and_unedited(false);
        // Pascal `BeginEdit` (DSSClass.pas:1666, r4086): capture the set-order
        // counter so `end_edit` knows which props this edit touched (RegControl's
        // signed-threshold legacy fallback needs it).
        active_arena[oi].data_mut().begin_edit_boundary();

        let mut param_pointer: i64 = 0;
        let mut param_name = parser.next_param(vars);
        let mut param = parser.make_string(vars);
        // Pascal `IsQuotedString` for the value token (r4133 per-phase switch
        // and relay state writers key on it — the quotes are stripped before
        // the property arm runs, so carry the flag beside the value;
        // [`PropEngine::was_quoted`]).
        let mut param_was_quoted = parser.is_quoted();
        while !param.is_empty() {
            if param_name.is_empty() {
                param_pointer += 1;
            } else {
                param_pointer = props
                    .property_index(&param_name)
                    .map(|i| i as i64)
                    .unwrap_or(0);
            }

            if param_pointer <= 0 || param_pointer as usize > props.num_properties() {
                // Not a class property, but may still be a dynamic-equation
                // variable for some classes (Pascal `DSSClass.Edit` l.1656 →
                // `Obj.ParseDynVar`).
                if active_arena[oi].parse_dyn_var(&param_name, &param, vars) {
                    // Consumed as a DynamicExp state-variable initializer.
                } else if param_name.is_empty() {
                    let from = errors.len();
                    errors.push(format!(
                        "Unknown parameter for value \"{param}\" in object \"{}.{}\"",
                        props.class_name(),
                        active_arena[oi].data().name()
                    ));
                    // P5b: underline the stray value token.
                    attach_source(
                        errors,
                        from,
                        parser.token_span(),
                        cmd_origin,
                        parser.cmd_string(),
                    );
                } else {
                    let from = errors.len();
                    errors.push(format!(
                        "Unknown parameter \"{param_name}\" (value \"{param}\") for object \"{}.{}\"",
                        props.class_name(),
                        active_arena[oi].data().name()
                    ));
                    // P5b: underline the unknown parameter *name* token.
                    attach_source(
                        errors,
                        from,
                        parser.param_name_span(),
                        cmd_origin,
                        parser.cmd_string(),
                    );
                }
            } else {
                let idx = param_pointer as usize;
                if props.prop(idx).ptype == PropType::MakeLike {
                    make_like(
                        active_arena,
                        name_to_idx,
                        oi,
                        &param,
                        errors,
                        props.class_name(),
                    );
                    active_arena[oi].data_mut().set_as_next_seq(idx);
                    active_arena[oi].side_effects(idx, 0);
                } else {
                    let from = errors.len();
                    let mut eng = PropEngine {
                        parser: aux_parser,
                        vars,
                        enums,
                        errors,
                        foreign: Some(&foreign),
                        was_quoted: param_was_quoted,
                    };
                    if let Err(e) =
                        props.edit_property(&mut active_arena[oi], idx, &param, &mut eng)
                    {
                        errors.push(e);
                    }
                    // P5b: every diagnostic this property edit produced — the
                    // bubbled conversion `Err` and any `DoSimpleMsg`-and-continue
                    // range/sign message pushed deep in `set_obj_*` — concerns the
                    // current value token; underline it against the command line.
                    attach_source(
                        errors,
                        from,
                        parser.token_span(),
                        cmd_origin,
                        parser.cmd_string(),
                    );
                }
            }

            param_name = parser.next_param(vars);
            param = parser.make_string(vars);
            param_was_quoted = parser.is_quoted();
        }

        // Pascal `TFuseObj.Create` resolves `FuseCurve := Find('tlink')` in the
        // constructor; our constructor cannot reach the registry, so resolve the
        // (default or explicit) curve name here through the same foreign view the
        // property edits use, cloning it into the Fuse for solve-time GetTCCTime.
        if let Some(name) = active_arena
            .get::<fuse::Fuse>(oi)
            .map(|f| f.fuse_curve_name().to_string())
        {
            let curve = (!name.is_empty())
                .then(|| {
                    foreign
                        .find("TCC_Curve", &name)
                        .and_then(|o| o.cloned::<tcc_curve::TccCurveObj>())
                })
                .flatten();
            if let Some(f) = active_arena.get_mut::<fuse::Fuse>(oi) {
                f.set_fuse_curve_obj(curve);
            }
        }

        // Resolve the harmonic spectrum for any element that injects from one
        // (Pascal `Set_Spectrum` / the constructor default `SpectrumObj :=
        // SpectrumClass.DefaultX`). The element reports its default/explicit
        // `spectrum=` name; we clone the resolved `SpectrumObj` (its `MultArray`
        // is already built by the spectrum's `EndEdit`) in for the harmonic
        // injection path. Same foreign-view pattern as the Fuse curve above.
        let spectrum_name = active_arena
            .try_ckt_elem(oi)
            .and_then(|ce| ce.harmonic_spectrum_name())
            .map(str::to_string);
        if let Some(name) = spectrum_name {
            let resolved = if name.is_empty() {
                None
            } else {
                let found = foreign
                    .find("Spectrum", &name)
                    .and_then(|o| o.cloned::<spectrum::SpectrumObj>());
                if found.is_none() {
                    // Pascal `Set_Spectrum` resolves a `DSSObjectReferenceProperty`
                    // and raises error 401 on a missing name — surface it loudly
                    // (else the element would inject silent-zero harmonic current).
                    errors.push(crate::diag::DssDiagnostic::msg(
                        format!(
                            "{}.{}.Spectrum: Spectrum object \"{name}\" not found.",
                            props.class_name(),
                            active_arena[oi].data().name()
                        ),
                        Some(401),
                    ));
                }
                found
            };
            if let Some(ce) = active_arena.try_ckt_elem_mut(oi) {
                ce.set_harmonic_spectrum(resolved);
            }
        }

        // Same pattern for the Recloser's four TCC curves (PhaseFast/PhaseDelayed
        // default to the built-in `a`/`d` in the constructor, which cannot reach
        // the registry): resolve every non-empty name through the foreign view and
        // clone the curves in for solve-time GetTCCTime.
        if let Some(names) = active_arena
            .get::<recloser::Recloser>(oi)
            .map(|r| r.curve_names())
        {
            let resolve = |name: &str| -> Option<tcc_curve::TccCurveObj> {
                (!name.is_empty())
                    .then(|| {
                        foreign
                            .find("TCC_Curve", name)
                            .and_then(|o| o.cloned::<tcc_curve::TccCurveObj>())
                    })
                    .flatten()
            };
            let curves = [
                resolve(&names[0]),
                resolve(&names[1]),
                resolve(&names[2]),
                resolve(&names[3]),
            ];
            if let Some(r) = active_arena.get_mut::<recloser::Recloser>(oi) {
                r.set_resolved_curves(curves);
            }
        }

        // Same pattern for the Relay's five TCC curves (PhaseCurve / GroundCurve
        // / OvervoltCurve / UndervoltCurve / DOC_PhaseCurveInner — all default to
        // NIL, but any `…curve=` parse names one): resolve through the foreign
        // view and clone in for solve-time GetTCCTime / GetOVtime / GetUVtime.
        if let Some(names) = active_arena
            .get::<relay::Relay>(oi)
            .map(|r| r.curve_names())
        {
            let resolve = |name: &str| -> Option<tcc_curve::TccCurveObj> {
                (!name.is_empty())
                    .then(|| {
                        foreign
                            .find("TCC_Curve", name)
                            .and_then(|o| o.cloned::<tcc_curve::TccCurveObj>())
                    })
                    .flatten()
            };
            let curves = [
                resolve(&names[0]),
                resolve(&names[1]),
                resolve(&names[2]),
                resolve(&names[3]),
                resolve(&names[4]),
            ];
            if let Some(r) = active_arena.get_mut::<relay::Relay>(oi) {
                r.set_resolved_curves(curves);
            }
        }

        // InvControl attaches its terminal to the first controlled DER's bus
        // (Pascal `RecalcElementData` runs `MakeDERList` + `Setbus(1,
        // MonitoredElement.Firstbus)`). `recalc_element_data` has no store access,
        // so resolve the first DER's bus + phase count here (the foreign view) and
        // hand them to the control; `end_edit` → `recalc` then attaches the
        // terminal. A named DERList resolves its first entry; an empty list scans
        // every PVSystem then Storage for the first enabled one (matching
        // `MakeDERList`'s empty-list branch).
        if active_arena.get::<inv_control::InvControl>(oi).is_some() {
            let der_names: Vec<String> = active_arena
                .get::<inv_control::InvControl>(oi)
                .map(|ic| ic.der_name_list().to_vec())
                .unwrap_or_default();
            let info: Option<(String, usize)> = {
                // Pascal `MonitoredElement := FDERPointerList.Get(1)` is the
                // first *enabled* member (the bus), but the recalc loop assigns
                // `FNphases := ControlledElement[i].NPhases` for EVERY member —
                // the LAST fleet member's phase count wins (InvControl.pas:916;
                // visible with a mixed 3ph+1ph fleet, midi_invcontrol).
                let (first, last): (
                    Option<&dyn crate::elements::traits::CktElement>,
                    Option<&dyn crate::elements::traits::CktElement>,
                ) = if der_names.is_empty() {
                    // Empty list = every PVSystem then every Storage.
                    (
                        foreign
                            .first_enabled("PVSystem")
                            .or_else(|| foreign.first_enabled("Storage")),
                        foreign
                            .last_enabled("Storage")
                            .or_else(|| foreign.last_enabled("PVSystem")),
                    )
                } else {
                    let resolve = |n: &String| {
                        let (class, name) = n.split_once('.').unwrap_or(("", n.as_str()));
                        foreign
                            .find(class, name)
                            .and_then(|o| o.ckt().filter(|e| e.cd().enabled))
                    };
                    (
                        der_names.iter().find_map(resolve),
                        der_names.iter().rev().find_map(resolve),
                    )
                };
                match (first, last) {
                    (Some(f), Some(l)) => Some((f.cd().get_bus(1).to_string(), l.cd().nphases)),
                    _ => None,
                }
            };
            if let Some((bus, nphases)) = info
                && let Some(ic) = active_arena.get_mut::<inv_control::InvControl>(oi)
            {
                ic.set_resolved_monitored(bus, nphases);
            }
        }

        // ExpControl attaches its terminal to the first controlled PVSystem's bus
        // (Pascal `RecalcElementData` runs `MakePVSystemList` + `Setbus(1,
        // MonitoredElement.Firstbus)`). Like InvControl, resolve the first PVSystem's
        // bus + phase count here (no store access in `recalc_element_data`). A named
        // list resolves its first *enabled* entry; an empty list scans every
        // PVSystem for the first enabled one (matching `MakePVSystemList`).
        if active_arena.get::<exp_control::ExpControl>(oi).is_some() {
            let pv_names: Vec<String> = active_arena
                .get::<exp_control::ExpControl>(oi)
                .map(|ec| ec.pvsystem_name_list().to_vec())
                .unwrap_or_default();
            let info: Option<(String, usize)> = {
                // Bus from the FIRST enabled member; phase count from the LAST
                // (the Pascal recalc loop assigns FNphases per member —
                // ExpControl.pas:408, same last-wins as InvControl).
                let resolve = |n: &String| {
                    foreign
                        .find("PVSystem", n)
                        .and_then(|o| o.ckt().filter(|e| e.cd().enabled))
                };
                let (first, last): (
                    Option<&dyn crate::elements::traits::CktElement>,
                    Option<&dyn crate::elements::traits::CktElement>,
                ) = if pv_names.is_empty() {
                    (
                        foreign.first_enabled("PVSystem"),
                        foreign.last_enabled("PVSystem"),
                    )
                } else {
                    (
                        pv_names.iter().find_map(resolve),
                        pv_names.iter().rev().find_map(resolve),
                    )
                };
                match (first, last) {
                    (Some(f), Some(l)) => Some((f.cd().get_bus(1).to_string(), l.cd().nphases)),
                    _ => None,
                }
            };
            if let Some((bus, nphases)) = info
                && let Some(ec) = active_arena.get_mut::<exp_control::ExpControl>(oi)
            {
                ec.set_resolved_monitored(bus, nphases);
            }
        }

        // GICsource splices itself into a Line with the SAME NAME (Pascal
        // `RecalcElementData` runs `LineClass.Find(Name)` then inserts a
        // `GIC_<name>` bus and rewrites the Line's Bus2). The constructor/recalc
        // have no store access, so resolve the Line by the GICsource's own name
        // through the foreign view here and hand it — with the Line's present
        // Bus2 — to the source; `end_edit` → `recalc` then decides whether to
        // splice and queues the Line Bus2 rewrite as a deferred RefAction.
        if active_arena.get::<gic_source::GicSource>(oi).is_some() {
            let name = active_arena[oi].data().name().to_string();
            let resolved = foreign.find("Line", &name).and_then(|o| {
                // `find("Line", …)` already fixes the class, so the narrowing
                // `idx::<Line>()` is total here — it only moves the class off
                // the value channel and into the type.
                o.ckt()
                    .zip(o.idx::<line::Line>())
                    .map(|(e, i)| (i, e.cd().get_bus(2).to_string()))
            });
            if let Some(gs) = active_arena.get_mut::<gic_source::GicSource>(oi) {
                gs.set_resolved_line(resolved);
            }
        }

        // Deferred file loads (Pascal runs `DoCSVFile` etc. in the property
        // hook, which has the DSS context; our hook cannot reach the filesystem
        // or the current directory, so it queues the request and we resolve it
        // here — before `end_edit`, so derived state like `SetMaxPandQ` sees the
        // loaded data). Paths resolve relative to `current_dir`, like Redirect;
        // the literal `%result%` resolves to `LastResultFile` (Pascal
        // `InterpretDblArray`, `Utilities.pas:464-465`).
        let resolve = |filename: &str| -> std::path::PathBuf {
            let name = if filename.eq_ignore_ascii_case("%result%") {
                last_result_file.as_str()
            } else {
                filename
            };
            let p = std::path::Path::new(name);
            if p.is_absolute() {
                p.to_path_buf()
            } else {
                current_dir.join(name)
            }
        };
        let file_loads = active_arena[oi].take_file_loads();
        for fl in &file_loads {
            let path = resolve(&fl.filename);
            if fl.binary {
                match std::fs::read(&path) {
                    Ok(bytes) => active_arena[oi].apply_binary_file_load(fl, &bytes, errors),
                    // Pascal error 615/617 (SngFile/DblFile "Error opening file").
                    Err(_) => errors.push(format!("Error opening file: \"{}\"", fl.filename)),
                }
            } else {
                match std::fs::read_to_string(&path) {
                    Ok(content) => active_arena[oi].apply_file_load(fl, &content, errors),
                    // Pascal error 613/58613 (CSVFile/PQCSVFile "Error opening file").
                    Err(_) => errors.push(format!("Error opening file: \"{}\"", fl.filename)),
                }
            }
        }

        // Deferred user-model (WASM) loads (WASM_USERMODELS WM.3+). Like the file
        // loads above, the `UserModel=`/`UserData=`/`ShaftModel=`/`ShaftData=`
        // property hook cannot reach the filesystem or the current directory, so
        // it queued the request; resolve the path here (literal → `current_dir`,
        // mirroring the Pascal `LoadLibrary(Value)` / `LoadLibrary(DSSDirectory +
        // Value)` order) and hand back the `.wasm` bytes — or `None`, which makes
        // the element warn "… Not Loaded" and fall back to the built-in model
        // (never "Error opening file", unlike a `FileLoad` miss). Runs before
        // `end_edit` so `RecalcElementData` sees the loaded model.
        // Pascal `RecalcElementData` and the user-model callbacks read the live
        // `ActiveCircuit.Solution` globals; snapshot them once for the user-model
        // load/edit below and the trailing `end_edit`. DSS_OBJECT edits (no circuit
        // yet) fall back to the fresh-circuit snapshot; those `end_edit`s ignore it.
        let live_sys = circuit
            .as_ref()
            .map(crate::solution::solution::sys_ctx)
            .unwrap_or_else(crate::elements::traits::SysCtx::parse_default);

        let user_model_loads = active_arena[oi].take_user_model_loads();
        for uml in &user_model_loads {
            let wasm: Option<Vec<u8>> = match &uml.action {
                crate::obj::base::UserModelAction::Load(name) => {
                    let path = resolve(name);
                    let is_wasm = path
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("wasm"));
                    if is_wasm && path.is_file() {
                        std::fs::read(&path).ok()
                    } else {
                        None
                    }
                }
                crate::obj::base::UserModelAction::Edit(_) => None,
            };
            active_arena[oi].apply_user_model_load(uml, wasm.as_deref(), &live_sys, errors);
        }

        // WPG.19: generic file-backed numeric-array directives (`%mag=(file=…)`,
        // `Yarray=(sngfile=…)`) queued by the generic double-array property path
        // (Pascal `DSSObjectHelper.pas:616-636`). Read the file and apply the
        // `InterpretDblArray` grammar (short-file shrink + `Round`/scale/non-zero)
        // through the object's typed accessors.
        let generic_files = active_arena[oi].take_generic_dbl_array_files();
        for gf in &generic_files {
            let path = resolve(&gf.filename);
            match std::fs::read(&path) {
                Ok(bytes) => {
                    apply_generic_dbl_array_file(&mut active_arena[oi], gf, &bytes, errors)
                }
                // Pascal error 70401 (`InterpretDblArray`: "CSV file could not be
                // opened") / 70501 / 70502.
                Err(_) => errors.push(format!("File \"{}\" could not be opened.", gf.filename)),
            }
        }

        // WPG.19: actions the object deferred until its file loads resolved
        // (LoadShape `action=normalize`/`ln`; Pascal runs it inline right after
        // the file read). Run before `end_edit` so `SetMaxPandQ` sees normalized
        // data.
        active_arena[oi].run_deferred_actions(errors);

        // Deferred binary shape saves (LoadShape/TShape/PriceShape
        // `Action=SngSave/DblSave`): the `Action` property hook cannot reach
        // `OutputDirectory`/`GlobalResult`, so it queued the write (Pascal
        // `SaveToDblFile`/`SaveToSngFile`). Perform it now, after `output_directory`
        // is reachable. Writes are pure little-endian IEEE-754 streams into the
        // output directory, exactly like Pascal `GetOutputStreamEx(FName, fmCreate)`.
        let shape_saves = active_arena[oi].take_shape_saves();
        for ss in &shape_saves {
            write_shape_save(output_directory, last_result, ss, errors);
        }

        // Deferred debug-trace creates (Storage `DebugTrace=yes`): like the shape
        // saves above, the property hook cannot reach `OutputDirectory`, so it
        // queued the file name + header (built with the phase/variable counts of
        // that instant, as Pascal's `CASE` arm does) and the object writes and
        // closes the file here — before `end_edit`, matching Pascal's in-hook
        // `AssignFile`/`ReWrite` (r4133 `PCElements/Storage.pas:1073-1085`).
        active_arena[oi].open_debug_traces(output_directory, errors);

        // Thread the live snapshot (built above) into `end_edit` so the
        // side-effect `RecalcElementData` runs against the current circuit state.
        active_arena[oi].end_edit(&live_sys);

        // The post-`end_edit` signal tail (deferred errors/abort, circuit
        // signal-flag propagation, deferred ref-actions). Shared verbatim with
        // the MakePosSequence applier (`exec/make_pos_seq.rs`), which replays
        // the identical property mutations through the typed setters. The split
        // active-class borrows above are dead by here (last used at `end_edit`),
        // so the full `classes` slice is free for the ref-action targets.
        apply_edit_signal_tail(classes, circuit, errors, ci, oi);
        // …and, for a control, the `Set_ControlledElement` re-attach that ends
        // its `RecalcElementData`. Outside the shared tail on purpose — the
        // MakePosSequence applier must NOT run it (see the function's doc).
        reattach_edited_control(classes, circuit, ci, oi);
    }
}

/// P5b: attach the executing command line as a diagnostic source, underlining
/// `span`, to every diagnostic pushed since `from` that has no source yet.
///
/// The executive owns the source origin (`"<command>"` or `"<file>:<line-no>"`)
/// and the offending token's byte range in the *main* command parser, so it is
/// authoritative: any span the property parser recorded on its scratch `(value)`
/// buffer is meaningless against this source and is overwritten. Diagnostics
/// that already carry a `src` (none do today) are left untouched. miette renders
/// the underline only when both `source_code()` and `labels()` are present.
fn attach_source(
    errors: &mut crate::diag::ErrorLog,
    from: usize,
    span: std::ops::Range<usize>,
    origin: &str,
    source: &str,
) {
    for d in errors.iter_mut().skip(from) {
        if d.src.is_none() {
            d.span = Some((span.start, span.len()).into());
            d.src = Some(miette::NamedSource::new(origin, source.to_string()));
        }
    }
}

/// The tail every property edit runs after `EndEdit` (Pascal: the property
/// setters write `ActiveCircuit.BusNameRedefined`/`Solution.SystemYChanged`
/// immediately; here they queue signal flags drained once the edit finishes).
/// Factored out of [`Dss::edit_active_inner`] so the MakePosSequence applier
/// (`exec/make_pos_seq.rs`) runs the byte-identical tail after replaying an
/// element's [`PosSeqPlan`] — no duplicated logic. `ci`/`oi` name the class /
/// object just edited; `classes` is the full registry (for ref-action targets),
/// each class reaching its objects through `class.arena` (R1 ownership flip).
pub(super) fn apply_edit_signal_tail(
    classes: &mut [DssClass],
    circuit: &mut Option<Circuit>,
    errors: &mut crate::diag::ErrorLog,
    ci: usize,
    oi: usize,
) {
    // Drain any `DoSimpleMsg`/`DoErrorMsg` queued by the property hooks
    // (e.g. `LineCode.Kron` on a 1-phase code) into the engine error log.
    let deferred = classes[ci].arena[oi].data_mut().take_errors();
    errors.extend(deferred);

    // A `DoErrorMsg`-class deferred message (e.g. Relay error 384, a
    // monitored terminal out of range) sets `DSS.SolutionAbort := True` in
    // Pascal; lift that request into the solution so the next solve halts.
    // `take_abort` always runs (clears the per-object flag); `DoSimpleMsg`
    // messages (errors 385/386) never set it.
    if classes[ci].arena[oi].data_mut().take_abort()
        && let Some(ckt) = circuit.as_mut()
    {
        ckt.solution.solution_abort = true;
    }

    // Deferred cross-element writes (Pascal pokes the target through a
    // live pointer mid-parse, e.g. RegControl `TapNum` → the transformer's
    // PresentTap; nothing reads the target in between, so applying after
    // the edit is equivalent).
    let ref_actions = classes[ci].arena[oi].take_ref_actions();

    // Signal-flag propagation (Pascal `Set_Bus`/`Set_Enabled` write the
    // circuit globals immediately; `Set_YprimInvalid` raises
    // `SystemYChanged` for enabled elements).
    if let Some(ckt) = circuit.as_mut()
        && let Some(elem) = classes[ci].arena.try_ckt_elem_mut(oi)
    {
        let cd = elem.cd_mut();
        if cd.signal_bus_name_redefined {
            cd.signal_bus_name_redefined = false;
            ckt.set_bus_name_redefined(true);
        }
        if cd.yprim_invalid && cd.enabled {
            ckt.solution.system_y_changed = true;
        }
        if cd.signal_reset_solution_initialized {
            cd.signal_reset_solution_initialized = false;
            ckt.solution.solution_initialized = false;
        }
    }

    for action in &ref_actions {
        let target = action.target();
        let tgt_arena = &mut classes[target.class_ord()].arena;
        let ti = target.index();
        // `SetSwitchClosed`/`SetConductorsClosed` act on the generic
        // CktElement base (any switched element), so they are applied here
        // rather than through the per-class `apply_ref_action`; the
        // transformer-tap variant stays class-specific.
        match action {
            crate::obj::base::RefAction::SetSwitchClosed {
                terminal, closed, ..
            } => {
                if let Some(elem) = tgt_arena.try_ckt_elem_mut(ti) {
                    elem.cd_mut().set_terminal_closed(*terminal, *closed);
                }
            }
            crate::obj::base::RefAction::SetConductorsClosed {
                terminal, closed, ..
            } => {
                if let Some(elem) = tgt_arena.try_ckt_elem_mut(ti) {
                    let cd = elem.cd_mut();
                    for (i, &c) in closed.iter().enumerate() {
                        cd.set_conductor_closed(*terminal, i + 1, c);
                    }
                }
            }
            // GICsource splice: rewrite the spliced Line's Bus2 to the
            // inserted GIC_<name> bus (Pascal drives it through the Line's
            // property path; the Bus2 side effect is a plain rename).
            crate::obj::base::RefAction::SetElementBus { terminal, bus, .. } => {
                if let Some(elem) = tgt_arena.try_ckt_elem_mut(ti) {
                    elem.cd_mut().set_bus(*terminal, bus);
                }
            }
            // OCP-device flags for the reliability sweep (Pascal
            // `Include(ControlledElement.Flags, Flg.HasOCPDevice)` in the
            // control's RecalcElementData). The first OCP control registered
            // wins the `GetOCPDeviceType` ordinal, mirroring the Pascal scan
            // that stops at the first Fuse/Recloser/Relay in the list.
            crate::obj::base::RefAction::SetOcpDevice {
                device_type, auto, ..
            } => {
                if let Some(elem) = tgt_arena.try_ckt_elem_mut(ti) {
                    let cd = elem.cd_mut();
                    cd.flags
                        .include(crate::elements::ckt::ElemFlags::HAS_OCP_DEVICE);
                    if *auto {
                        cd.flags
                            .include(crate::elements::ckt::ElemFlags::HAS_AUTO_OCP_DEVICE);
                    }
                    if cd.ocp_device_type == crate::elements::ckt::OcpDeviceType::Unset {
                        cd.ocp_device_type = *device_type;
                    }
                }
            }
            _ => tgt_arena.obj_mut(ti).apply_ref_action(action),
        }
        // Propagate the target's flags too (a tap change invalidates the
        // transformer's Yprim exactly like a direct `Tap=` edit).
        if let Some(ckt) = circuit.as_mut()
            && let Some(elem) = tgt_arena.try_ckt_elem_mut(ti)
        {
            let cd = elem.cd_mut();
            if cd.signal_bus_name_redefined {
                cd.signal_bus_name_redefined = false;
                ckt.set_bus_name_redefined(true);
            }
            if cd.yprim_invalid && cd.enabled {
                ckt.solution.system_y_changed = true;
            }
        }
    }
}

/// The control half of Pascal `RecalcElementData`: it ends by re-assigning
/// `ControlledElement`, which is the property setter
/// `TControlElem.Set_ControlledElement` (r4133 `Controls/ControlElem.pas:113-131`):
/// remove self from the previous target's `ControlElementList`, append to the new
/// target's. r4133 runs it on EVERY edit (`Controls/Relay.pas:955` from `:626`;
/// `Recloser.pas:702`, `SwtControl.pas:332`, `CapControl.pas:580`,
/// `RegControl.pas:693`, `Controls/fuse.pas`), so a re-edited control moves to the
/// end of its element's list. capi 0.14.5 instead makes `ControlledElement` a
/// property-write target (`Controls/Relay.pas:439-441`) and leaves the order alone
/// on a re-edit — r4133 is the behavioral authority (`DIVERGENCES.md`).
///
/// Call it where the edit is over (`end_edit` = `RecalcElementData` has run, and
/// so have the deferred ref-actions of [`apply_edit_signal_tail`]), so
/// `controlled_element()` is the control's final target for this edit. Guarded on
/// the class kind — only a `TControlElem` has a `ControlElementList` membership to
/// maintain.
///
/// **Deliberately not part of [`apply_edit_signal_tail`]** (G1.3d(ii) audit
/// settlement, 2026-09-05): the MakePosSequence applier
/// (`exec/make_pos_seq.rs`) shares that tail, and r4133's `MakePosSequence` never
/// re-runs `RecalcElementData` on a control — every control override mutates its
/// own fields and ends with `inherited`, the base bus rename
/// (`Common/CktElement.pas:1352`): `Controls/Relay.pas:1008`, `Recloser.pas:738`,
/// `SwtControl.pas:367`, `CapControl.pas:656`, `RegControl.pas:1491`, and `Fuse`
/// has no override at all. `DoMakePosSeq` calls nothing else
/// (`Executive/ExecHelper.pas:3069-3086`), so a `MakePosSeq` leaves every
/// `ControlElementList` order untouched. Pinned by
/// `exec::tests::element_extras::makeposseq_does_not_reattach_controls`.
pub(super) fn reattach_edited_control(
    classes: &mut [DssClass],
    circuit: &mut Option<Circuit>,
    ci: usize,
    oi: usize,
) {
    if classes[ci].kind == Some(crate::circuit::ElemKind::Control)
        && let Some(ckt) = circuit.as_mut()
    {
        let attached = classes[ci]
            .arena
            .try_ckt_elem(oi)
            .and_then(|ce| ce.controlled_element())
            .is_some();
        ckt.reattach_control(classes[ci].arena.id(oi), attached);
    }
}

/// Apply a resolved generic file-backed numeric-array directive (WPG.19, Pascal
/// `DSSObjectHelper.pas:616-636`): read the file with the `InterpretDblArray`
/// grammar (short-file shrink), then re-apply `Round`/scale/non-zero exactly like
/// the inline list path (`parse.rs`), and write the array + shrunk count through
/// the object's typed accessors. The read is capped at the current count
/// property so Pascal's in-place shrink of one array is visible to a later one
/// (e.g. `%mag` shrinking `NumHarm` before `angle` reads).
fn apply_generic_dbl_array_file(
    obj: &mut dyn crate::obj::base::DssObject,
    gf: &crate::obj::base::GenericDblArrayFile,
    bytes: &[u8],
    errors: &mut crate::diag::ErrorLog,
) {
    use crate::obj::base::MmfKind;
    let max = obj.get_i32(gf.size_prop).max(0) as usize;
    let mut vals = match gf.kind {
        MmfKind::Text => {
            let content = String::from_utf8_lossy(bytes);
            let (vals, err_row) =
                crate::util::read_dbl_array_text(&content, gf.column, gf.header, max);
            if let Some(row) = err_row {
                // Pascal `DoSimpleMsg(#705)` then stop-and-shrink
                // (`Utilities.pas:515-521`); `vals` already holds only `i-1`.
                errors.push(crate::diag::DssDiagnostic::msg(
                    format!(
                        "{}: (#705) Error reading {row}-th numeric array value from file.",
                        obj.data().name()
                    ),
                    Some(705),
                ));
            }
            vals
        }
        MmfKind::Float32 => crate::util::read_le_f32_array(bytes, max),
        MmfKind::Float64 => crate::util::read_le_f64_array(bytes, max),
    };
    if gf.apply_round {
        // Pascal `Round` = ties-to-even (see the inline list path in
        // `class_props/parse.rs`); array magnitudes are always in Int64 range.
        for v in &mut vals {
            *v = v.round_ties_even();
        }
    }
    if gf.non_zero && vals.contains(&0.0) {
        errors.push(format!(
            "{}: file-backed array elements cannot be zero.",
            obj.data().name()
        ));
        return;
    }
    if gf.scale != 1.0 {
        for v in &mut vals {
            *v *= gf.scale;
        }
    }
    let count = vals.len() as i32;
    obj.set_f64_array(gf.prop, vals);
    // Pascal `integerPtr^ := InterpretDblArray(...)`: shrink the count property to
    // the number of values read.
    obj.set_i32(gf.size_prop, count);
}

/// Write a queued [`ShapeSave`] to `OutputDirectory` and set `GlobalResult`
/// (Pascal `TLoadShapeObj.SaveToDblFile`/`SaveToSngFile` and the TShape/
/// PriceShape equivalents). The P/value file is always written; the Q file only
/// when the shape carries a Q series (`if Assigned(dQ)`). Filenames follow the
/// class convention: LoadShape splits `<name>_P`/`<name>_Q`, TShape/PriceShape
/// use the bare `<name>`. The streams are raw little-endian IEEE-754.
fn write_shape_save(
    output_directory: &Path,
    last_result: &mut String,
    ss: &crate::obj::base::ShapeSave,
    errors: &mut crate::diag::ErrorLog,
) {
    let ext = if ss.sng { "sng" } else { "dbl" };
    let ftag = if ss.sng { "sngfile" } else { "dblfile" };

    let p_name = if ss.p_suffix {
        format!("{}_P.{ext}", ss.name)
    } else {
        format!("{}.{ext}", ss.name)
    };
    let p_path = output_directory.join(&p_name);
    if let Err(e) = std::fs::write(&p_path, encode_shape_bytes(&ss.values, ss.sng)) {
        errors.push(format!(
            "Error writing file: \"{}\" ({e})",
            p_path.display()
        ));
        return;
    }
    // Pascal `DSS.GlobalResult := '<tag>=[<ftag>=' + FName + ']'`.
    *last_result = format!("{}=[{ftag}={}]", ss.result_tag, p_path.display());

    // Q file (LoadShape only, and only when `dQ` is assigned).
    if let Some(q) = &ss.q_values {
        let q_path = output_directory.join(format!("{}_Q.{ext}", ss.name));
        if let Err(e) = std::fs::write(&q_path, encode_shape_bytes(q, ss.sng)) {
            errors.push(format!(
                "Error writing file: \"{}\" ({e})",
                q_path.display()
            ));
            return;
        }
        // Pascal `AppendGlobalResult(DSS, ' Qmult=[<ftag>=' + FName + ']')` —
        // `AppendGlobalResult` (`DSSGlobals.pas:452-459`) joins a non-empty
        // result with `', '`, and the appended clause itself starts with a
        // space, so the oracle emits `],  Qmult=[` (comma + TWO spaces; audit
        // settlement, oracle-probed).
        last_result.push_str(&format!(",  Qmult=[{ftag}={}]", q_path.display()));
    }
}

/// Serialize a shape series to a raw little-endian byte stream: f32 for `sng`,
/// f64 otherwise (Pascal `F.Write(Single/Double)`).
fn encode_shape_bytes(values: &[f64], sng: bool) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(values.len() * if sng { 4 } else { 8 });
    for &v in values {
        if sng {
            bytes.extend_from_slice(&(v as f32).to_le_bytes());
        } else {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
    }
    bytes
}
