//! The command dispatcher (`ProcessCommand`) plus object lifecycle and
//! editing (`New`/`Edit`/`~`/`Clear`/`?` and `TDSSClass.Edit`).
//! Split out of `exec/mod.rs`.

use super::*;

impl Dss {
    /// Process one command line (Pascal `ProcessCommand`). Errors are recorded
    /// in [`Dss::errors`] (record-and-continue); query results land in
    /// [`Dss::result`].
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
            cmd::FILEEDIT
            | cmd::CLASSES
            | cmd::USERCLASSES
            | cmd::ALIGN_FILE
            | cmd::DI_PLOT
            | cmd::COMPARE_CASES
            | cmd::YEARLY_CURVES
            | cmd::CD
            | cmd::DOSCMD
            | cmd::CVRT_LOADSHAPES
            | cmd::VAR => {
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
                self.errors.push(format!("Unknown Command: \"{param}\""));
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
            cmd::SOLVE => self.do_set_cmd(1), // Solve = Set + DoSolveCmd
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
            // Pascal `DoPlotCmd`/`DoVisualizeCmd` are GUI commands; in the pinned
            // headless oracle they produce no engine-observable state, so a
            // documented no-op is faithful, not a fake (PHASE8_PLAN §2.5; the live
            // gate compares the assembled model, not any plot). Post-circuit, like
            // the oracle's dispatch (so before a circuit the generic guard above
            // emits #301; oracle-probed). NOT_PORTED, tracked for a real WP:
            // `Visualize` on an *unsolved* circuit errors #24722 on the oracle —
            // not reproduced here (no corpus deck reaches it; Visualize stays a
            // documented no-op per §2.5).
            cmd::PLOT | cmd::VISUALIZE => {}
            cmd::BATCH_EDIT => self.do_batch_edit_cmd(),
            // Pascal `ExecCommands.pas` `ord(Cmd.MakeBusList)`:
            // `if BusNameRedefined then ReprocessBusDefs` — nothing else.
            cmd::MAKE_BUS_LIST => self.do_make_bus_list_cmd(),
            // Pascal `ExecCommands.pas` `ord(Cmd.GISCoords)`: "Do nothing here
            // on DSS C-API. Just ignore it silently so files saved with EPRI's
            // version can be loaded more easily." (OpenDSS-GIS is out of the
            // DSS-Extensions scope.)
            cmd::GIS_COORDS => {}
            cmd::SET_BUS_XY => self.do_set_bus_xy_cmd(),
            cmd::INTERPOLATE => self.do_interpolate_cmd(),
            // Pascal `DoRemoveCmd` (ExecHelper.pas:4939) → `DoRemoveBranches`.
            cmd::REMOVE => self.do_remove_cmd(),
            cmd::INIT => {
                if let Some(ckt) = self.circuit.as_mut() {
                    ckt.solution.solution_initialized = false;
                }
            }
            _ => self.not_ported_command(pointer),
        }
    }

    fn not_ported_command(&mut self, pointer: usize) {
        let name = EXEC_COMMANDS.get(pointer - 1).copied().unwrap_or("?");
        self.errors
            .push(format!("Command \"{name}\" is not ported yet."));
    }

    /// Pascal `GetObjClassAndName`: read the `class.name` token (optionally
    /// prefixed `object=`) from the main parser.
    fn get_obj_class_and_name(&mut self) -> (String, String) {
        let param_name = self.parser.next_param(&self.vars).to_lowercase();
        let param = self.parser.make_string(&self.vars);
        if !param_name.is_empty() && !crate::util::compare_text_shortest_eq(&param_name, "object") {
            // Pascal error 240: the `%s` argument is `CRLF + Parser.CmdString`
            // (`sLineBreak`, rendered LF here like every other output line).
            self.errors.push(format!(
                "object=Class.Name expected as first parameter in command. \n{}",
                self.parser.cmd_string()
            ));
            return (String::new(), String::new());
        }
        parse_object_class_and_name(&mut self.parser, &self.vars, &param)
    }

    /// Pascal `DSSGlobals.SetObject`: set the active object by `class.name`
    /// (or bare `name` against the active class).
    fn set_object(&mut self, param: &str) -> bool {
        let (class_part, name_part) = match param.find('.') {
            Some(p) => (param[..p].to_string(), param[p + 1..].to_string()),
            None => (String::new(), param.to_string()),
        };
        let ci = if class_part.is_empty() {
            self.active_class
        } else {
            self.class_by_name.get(&class_part.to_lowercase()).copied()
        };
        let Some(ci) = ci else {
            self.errors
                .push(format!("Error! Object \"{param}\" not found."));
            return false;
        };
        self.active_class = Some(ci);
        if !self.classes[ci].set_active(&name_part) {
            self.errors
                .push(format!("Error! Object \"{param}\" not found."));
            return false;
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
        let Some(&ci) = self.class_by_name.get(&obj_class.to_lowercase()) else {
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

    /// Pascal `DoBatchEditCmd` (`ExecHelper.pas:292`):
    /// `BatchEdit class.pattern editstring` — replay the trailing edit string
    /// against every object of the class whose NAME matches the regex pattern
    /// **case-insensitively and unanchored** (`TRegExpr` `ModifierI` + `Exec`
    /// = search anywhere in the name). The parser position at the start of the
    /// edit string is remembered and rewound for each match, exactly like the
    /// Pascal `Params := Parser.Position` / `Parser.Position := Params` dance.
    /// The command always returns 0 silently — there is no count message.
    fn do_batch_edit_cmd(&mut self) {
        let (obj_class, pattern) = self.get_obj_class_and_name();
        if obj_class.eq_ignore_ascii_case("circuit") {
            return; // Do nothing
        }
        let Some(&ci) = self.class_by_name.get(&obj_class.to_lowercase()) else {
            // Pascal error 267 (the `%s` is `CRLF + Parser.CmdString`; LF here,
            // same rendering as error 240 in `get_obj_class_and_name`).
            self.errors.push(format!(
                "BatchEdit Command: Object Type \"{obj_class}\" not found. \n{}",
                self.parser.cmd_string()
            ));
            return;
        };
        self.active_class = Some(ci); // DSS.LastClassReferenced / ActiveDSSClass
        // `Params := DSS.Parser.Position` — the edit string starts here.
        let params_pos = self.parser.position();
        let re = match regex::RegexBuilder::new(&pattern)
            .case_insensitive(true) // TRegExpr `ModifierI := TRUE`
            .build()
        {
            Ok(re) => re,
            Err(e) => {
                // TRegExpr raises `ERegExpr` on a bad pattern, surfaced as an
                // engine error by the executive's exception handler; the exact
                // upstream text is FPC-internal, so record the regex error.
                self.errors.push(format!("BatchEdit Command: {e}"));
                return;
            }
        };
        // `First`/`Next`: walk the class list in creation order; every visited
        // object becomes the active one (matching or not), the edit runs only
        // on a regex match.
        for oi in 0..self.classes[ci].objects.len() {
            self.classes[ci].active = Some(oi);
            let name = self.classes[ci].objects[oi].data().name().to_string();
            if re.is_match(&name) {
                self.parser.set_position(params_pos);
                self.edit_active();
            }
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
            let cd = self.classes[ci].objects[idx]
                .as_ckt_element_mut()
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
        let Some(&ci) = self.class_by_name.get(&obj_class.to_lowercase()) else {
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
        if self.classes[ci].objects[idx].as_ckt_element().is_none() {
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
            match self.class_by_name.get(&obj_class.to_lowercase()).copied() {
                Some(ci) => self.active_class = Some(ci),
                None => self
                    .errors
                    .push(format!("Error! Object Class \"{obj_class}\" not found. ")),
            }
        }
        // Pascal `ActiveDSSClass := Get(LastClassReferenced)` = our `active_class`;
        // `NIL` (no class ever referenced) → #246.
        let Some(ci) = self.active_class else {
            self.errors
                .push("Error! Active object type/class is not set.".to_string());
            return;
        };
        if !self.classes[ci].set_active(&obj_name) {
            // Pascal #245.
            self.errors
                .push(format!("Error! Object \"{obj_name}\" not found. "));
            return;
        }
        let idx = self.classes[ci]
            .active
            .expect("set_active set the active index");
        // Only circuit elements become the `ActiveCktElement` (Pascal: a general
        // `DSS_OBJECT` does nothing here).
        if self.classes[ci].objects[idx].as_ckt_element().is_some() {
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
            let cd = self.classes[ci].objects[idx]
                .as_ckt_element_mut()
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

    /// Pascal `AddObject`: create the object (or make the existing one
    /// active for `DSS_OBJECT` classes), register circuit elements with the
    /// circuit, and edit the rest of the line.
    pub(super) fn add_object(&mut self, obj_class: &str, name: &str) {
        let Some(&ci) = self.class_by_name.get(&obj_class.to_lowercase()) else {
            self.errors.push(format!(
                "New Command: Object Type \"{obj_class}\" not found."
            ));
            return;
        };
        self.active_class = Some(ci);

        if name.is_empty() {
            self.errors.push("Object Name Missing".to_string());
            return;
        }

        if self.classes[ci].requires_circuit && self.circuit.is_none() {
            self.errors
                .push("You Must Create a circuit first: \"new circuit.yourcktname\"".to_string());
            return;
        }

        if !self.classes[ci].requires_circuit {
            // DSS_OBJECT path: duplicates become edits.
            if !self.classes[ci].set_active(name) {
                let cls = &mut self.classes[ci];
                let obj = (cls.new_object)(name);
                let idx = cls.objects.len();
                cls.name_to_idx.insert(obj.data().name().to_string(), idx);
                cls.objects.push(obj);
                cls.active = Some(idx);
                // Pascal `DSS.DSSObjs.Add(Obj)` (`ExecHelper.pas:1899`): the
                // global creation-order list the whole-circuit Dump walks.
                self.dss_objs.push(ElemRef { cls: ci, idx });
            }
            self.edit_active();
            return;
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
            return;
        }

        let cls = &mut self.classes[ci];
        let obj = (cls.new_object)(name);
        let idx = cls.objects.len();
        cls.name_to_idx.insert(obj.data().name().to_string(), idx);
        cls.objects.push(obj);
        cls.active = Some(idx);

        // Pascal `TLineObj.Create` copies the context default earth model into
        // `FEarthModel` (Line.pas:998); a later `EarthModel=` edit can override
        // it. Applied before `edit_active` so the property still wins.
        if let Some(line) = self.classes[ci].objects[idx]
            .as_any_mut()
            .downcast_mut::<line::Line>()
        {
            line.earth_model = self.default_earth_model;
        }

        let cls = &mut self.classes[ci];
        let kind = cls.kind.expect("circuit element class has a kind");
        let ckt = self.circuit.as_mut().expect("checked above");
        let elem = self.classes[ci].objects[idx]
            .as_ckt_element_mut()
            .expect("circuit element class builds circuit elements");
        ckt.add_ckt_element(ElemRef { cls: ci, idx }, kind, elem);

        self.edit_active();
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
        for cls in &mut self.classes {
            cls.objects.clear();
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
        let Some(&ci) = self.class_by_name.get(&class_name.to_lowercase()) else {
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
            self.last_result = self.classes[ci].props.get_value(
                self.classes[ci].objects[oi].as_ref(),
                idx,
                &self.enums,
            );
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
    pub(super) fn refresh_vterminal_if_marked(
        &mut self,
        ci: usize,
        oi: usize,
        prop_idx: Option<usize>,
    ) {
        use crate::obj::props::PropFlags;
        let props = &self.classes[ci].props;
        let marked = match prop_idx {
            Some(i) => props.prop(i).flags.contains(PropFlags::READS_VTERMINAL),
            None => (1..=props.num_properties())
                .any(|i| props.prop(i).flags.contains(PropFlags::READS_VTERMINAL)),
        };
        if marked
            && let Some(node_v) = self.circuit.as_ref().map(|c| c.solution.node_v.clone())
            && let Some(elem) = self.classes[ci].objects[oi].as_ckt_element_mut()
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
            && let Some(tref) = self.classes[ci].objects[oi]
                .as_any()
                .downcast_ref::<reg_control::RegControl>()
                .and_then(|rc| rc.controlled_ref())
        {
            let rc_ref = ElemRef { cls: ci, idx: oi };
            let mut store = ClassStore {
                classes: &mut self.classes,
            };
            let (rc_obj, tr_obj) = store.pair_mut(rc_ref, tref);
            if let (Some(rc), Some(tr)) = (
                rc_obj
                    .as_any_mut()
                    .downcast_mut::<reg_control::RegControl>(),
                // Either member of the Transformer/AutoTrans proxy.
                transformer::as_controlled_transformer(&*tr_obj),
            ) {
                rc.sync_tap_snap_from_live(tr);
            }
        }
    }

    /// The body of Pascal `TDSSClass.Edit`: iterate `name=value` parameters on
    /// the main parser against the active object, then `EndEdit`, then
    /// propagate the element's signal flags to the circuit (the Pascal set
    /// `ActiveCircuit.BusNameRedefined`/`Solution.SystemYChanged` directly
    /// from the property setters; nothing reads them mid-edit, so polling
    /// after the edit is equivalent).
    pub(super) fn edit_active(&mut self) {
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
            ..
        } = self;
        // Split the registry so the active class is borrowed mutably for the
        // edit while every *other* class is a read view for ObjectRef
        // resolution (PHASE4_PLAN §3.1). `split_at_mut` + `split_first_mut`
        // keep the three regions provably disjoint with no unsafe.
        let (left, rest) = classes.split_at_mut(ci);
        let (active_class, right) = rest.split_first_mut().expect("ci is in range");
        let foreign = ForeignClasses {
            left,
            right,
            split: ci,
        };
        let DssClass {
            props,
            objects,
            name_to_idx,
            active,
            ..
        } = active_class;
        let Some(oi) = *active else {
            errors.push("There is no active element to edit.".to_string());
            return;
        };

        let mut param_pointer: i64 = 0;
        let mut param_name = parser.next_param(vars);
        let mut param = parser.make_string(vars);
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
                if objects[oi].parse_dyn_var(&param_name, &param, vars) {
                    // Consumed as a DynamicExp state-variable initializer.
                } else if param_name.is_empty() {
                    errors.push(format!(
                        "Unknown parameter for value \"{param}\" in object \"{}.{}\"",
                        props.class_name(),
                        objects[oi].data().name()
                    ));
                } else {
                    errors.push(format!(
                        "Unknown parameter \"{param_name}\" (value \"{param}\") for object \"{}.{}\"",
                        props.class_name(),
                        objects[oi].data().name()
                    ));
                }
            } else {
                let idx = param_pointer as usize;
                if props.prop(idx).ptype == PropType::MakeLike {
                    make_like(objects, name_to_idx, oi, &param, errors, props.class_name());
                    objects[oi].data_mut().set_as_next_seq(idx);
                    objects[oi].side_effects(idx, 0);
                } else {
                    let mut eng = PropEngine {
                        parser: aux_parser,
                        vars,
                        enums,
                        errors,
                        foreign: Some(&foreign),
                    };
                    if let Err(e) = props.edit_property(objects[oi].as_mut(), idx, &param, &mut eng)
                    {
                        errors.push(e.message().to_string());
                    }
                }
            }

            param_name = parser.next_param(vars);
            param = parser.make_string(vars);
        }

        // Pascal `TFuseObj.Create` resolves `FuseCurve := Find('tlink')` in the
        // constructor; our constructor cannot reach the registry, so resolve the
        // (default or explicit) curve name here through the same foreign view the
        // property edits use, cloning it into the Fuse for solve-time GetTCCTime.
        if let Some(name) = objects[oi]
            .as_any()
            .downcast_ref::<fuse::Fuse>()
            .map(|f| f.fuse_curve_name().to_string())
        {
            let curve = (!name.is_empty())
                .then(|| {
                    foreign.find("TCC_Curve", &name).and_then(|(_, o)| {
                        o.as_any().downcast_ref::<tcc_curve::TccCurveObj>().cloned()
                    })
                })
                .flatten();
            if let Some(f) = objects[oi].as_any_mut().downcast_mut::<fuse::Fuse>() {
                f.set_fuse_curve_obj(curve);
            }
        }

        // Resolve the harmonic spectrum for any element that injects from one
        // (Pascal `Set_Spectrum` / the constructor default `SpectrumObj :=
        // SpectrumClass.DefaultX`). The element reports its default/explicit
        // `spectrum=` name; we clone the resolved `SpectrumObj` (its `MultArray`
        // is already built by the spectrum's `EndEdit`) in for the harmonic
        // injection path. Same foreign-view pattern as the Fuse curve above.
        let spectrum_name = objects[oi]
            .as_ckt_element()
            .and_then(|ce| ce.harmonic_spectrum_name())
            .map(str::to_string);
        if let Some(name) = spectrum_name {
            let resolved = if name.is_empty() {
                None
            } else {
                let found = foreign
                    .find("Spectrum", &name)
                    .and_then(|(_, o)| o.as_any().downcast_ref::<spectrum::SpectrumObj>().cloned());
                if found.is_none() {
                    // Pascal `Set_Spectrum` resolves a `DSSObjectReferenceProperty`
                    // and raises error 401 on a missing name — surface it loudly
                    // (else the element would inject silent-zero harmonic current).
                    errors.push(format!(
                        "{}.{}.Spectrum: Spectrum object \"{name}\" not found.",
                        props.class_name(),
                        objects[oi].data().name()
                    ));
                }
                found
            };
            if let Some(ce) = objects[oi].as_ckt_element_mut() {
                ce.set_harmonic_spectrum(resolved);
            }
        }

        // Same pattern for the Recloser's four TCC curves (PhaseFast/PhaseDelayed
        // default to the built-in `a`/`d` in the constructor, which cannot reach
        // the registry): resolve every non-empty name through the foreign view and
        // clone the curves in for solve-time GetTCCTime.
        if let Some(names) = objects[oi]
            .as_any()
            .downcast_ref::<recloser::Recloser>()
            .map(|r| r.curve_names())
        {
            let resolve = |name: &str| -> Option<tcc_curve::TccCurveObj> {
                (!name.is_empty())
                    .then(|| {
                        foreign.find("TCC_Curve", name).and_then(|(_, o)| {
                            o.as_any().downcast_ref::<tcc_curve::TccCurveObj>().cloned()
                        })
                    })
                    .flatten()
            };
            let curves = [
                resolve(&names[0]),
                resolve(&names[1]),
                resolve(&names[2]),
                resolve(&names[3]),
            ];
            if let Some(r) = objects[oi]
                .as_any_mut()
                .downcast_mut::<recloser::Recloser>()
            {
                r.set_resolved_curves(curves);
            }
        }

        // Same pattern for the Relay's five TCC curves (PhaseCurve / GroundCurve
        // / OvervoltCurve / UndervoltCurve / DOC_PhaseCurveInner — all default to
        // NIL, but any `…curve=` parse names one): resolve through the foreign
        // view and clone in for solve-time GetTCCTime / GetOVtime / GetUVtime.
        if let Some(names) = objects[oi]
            .as_any()
            .downcast_ref::<relay::Relay>()
            .map(|r| r.curve_names())
        {
            let resolve = |name: &str| -> Option<tcc_curve::TccCurveObj> {
                (!name.is_empty())
                    .then(|| {
                        foreign.find("TCC_Curve", name).and_then(|(_, o)| {
                            o.as_any().downcast_ref::<tcc_curve::TccCurveObj>().cloned()
                        })
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
            if let Some(r) = objects[oi].as_any_mut().downcast_mut::<relay::Relay>() {
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
        if objects[oi].as_any().is::<inv_control::InvControl>() {
            let der_names: Vec<String> = objects[oi]
                .as_any()
                .downcast_ref::<inv_control::InvControl>()
                .map(|ic| ic.der_name_list().to_vec())
                .unwrap_or_default();
            let info: Option<(String, usize)> = {
                // Pascal `MonitoredElement := FDERPointerList.Get(1)` is the
                // first *enabled* member (the bus), but the recalc loop assigns
                // `FNphases := ControlledElement[i].NPhases` for EVERY member —
                // the LAST fleet member's phase count wins (InvControl.pas:916;
                // visible with a mixed 3ph+1ph fleet, midi_invcontrol).
                let (first, last): (Option<&dyn DssObject>, Option<&dyn DssObject>) =
                    if der_names.is_empty() {
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
                            foreign.find(class, name).and_then(|(_, o)| {
                                let enabled = o.as_ckt_element().is_some_and(|e| e.cd().enabled);
                                enabled.then_some(o)
                            })
                        };
                        (
                            der_names.iter().find_map(resolve),
                            der_names.iter().rev().find_map(resolve),
                        )
                    };
                match (
                    first.and_then(|o| o.as_ckt_element()),
                    last.and_then(|o| o.as_ckt_element()),
                ) {
                    (Some(f), Some(l)) => Some((f.cd().get_bus(1).to_string(), l.cd().nphases)),
                    _ => None,
                }
            };
            if let Some((bus, nphases)) = info
                && let Some(ic) = objects[oi]
                    .as_any_mut()
                    .downcast_mut::<inv_control::InvControl>()
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
        if objects[oi].as_any().is::<exp_control::ExpControl>() {
            let pv_names: Vec<String> = objects[oi]
                .as_any()
                .downcast_ref::<exp_control::ExpControl>()
                .map(|ec| ec.pvsystem_name_list().to_vec())
                .unwrap_or_default();
            let info: Option<(String, usize)> = {
                // Bus from the FIRST enabled member; phase count from the LAST
                // (the Pascal recalc loop assigns FNphases per member —
                // ExpControl.pas:408, same last-wins as InvControl).
                let resolve = |n: &String| {
                    foreign.find("PVSystem", n).and_then(|(_, o)| {
                        let enabled = o.as_ckt_element().is_some_and(|e| e.cd().enabled);
                        enabled.then_some(o)
                    })
                };
                let (first, last): (Option<&dyn DssObject>, Option<&dyn DssObject>) =
                    if pv_names.is_empty() {
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
                match (
                    first.and_then(|o| o.as_ckt_element()),
                    last.and_then(|o| o.as_ckt_element()),
                ) {
                    (Some(f), Some(l)) => Some((f.cd().get_bus(1).to_string(), l.cd().nphases)),
                    _ => None,
                }
            };
            if let Some((bus, nphases)) = info
                && let Some(ec) = objects[oi]
                    .as_any_mut()
                    .downcast_mut::<exp_control::ExpControl>()
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
        if objects[oi].as_any().is::<gic_source::GicSource>() {
            let name = objects[oi].data().name().to_string();
            let resolved = foreign.find("Line", &name).and_then(|(r, o)| {
                o.as_ckt_element()
                    .map(|e| (r, e.cd().get_bus(2).to_string()))
            });
            if let Some(gs) = objects[oi]
                .as_any_mut()
                .downcast_mut::<gic_source::GicSource>()
            {
                gs.set_resolved_line(resolved);
            }
        }

        // Deferred file loads (Pascal runs `DoCSVFile` etc. in the property
        // hook, which has the DSS context; our hook cannot reach the filesystem
        // or the current directory, so it queues the request and we resolve it
        // here — before `end_edit`, so derived state like `SetMaxPandQ` sees the
        // loaded data). Paths resolve relative to `current_dir`, like Redirect.
        let file_loads = objects[oi].take_file_loads();
        for fl in &file_loads {
            let path = current_dir.join(&fl.filename);
            if fl.binary {
                match std::fs::read(&path) {
                    Ok(bytes) => objects[oi].apply_binary_file_load(fl, &bytes, errors),
                    // Pascal error 615/617 (SngFile/DblFile "Error opening file").
                    Err(_) => errors.push(format!("Error opening file: \"{}\"", fl.filename)),
                }
            } else {
                match std::fs::read_to_string(&path) {
                    Ok(content) => objects[oi].apply_file_load(fl, &content, errors),
                    // Pascal error 613/58613 (CSVFile/PQCSVFile "Error opening file").
                    Err(_) => errors.push(format!("Error opening file: \"{}\"", fl.filename)),
                }
            }
        }

        objects[oi].end_edit();

        // Drain any `DoSimpleMsg`/`DoErrorMsg` queued by the property hooks
        // (e.g. `LineCode.Kron` on a 1-phase code) into the engine error log.
        let deferred = objects[oi].data_mut().take_errors();
        errors.extend(deferred);

        // A `DoErrorMsg`-class deferred message (e.g. Relay error 384, a
        // monitored terminal out of range) sets `DSS.SolutionAbort := True` in
        // Pascal; lift that request into the solution so the next solve halts.
        // `take_abort` always runs (clears the per-object flag); `DoSimpleMsg`
        // messages (errors 385/386) never set it.
        if objects[oi].data_mut().take_abort()
            && let Some(ckt) = circuit.as_mut()
        {
            ckt.solution.solution_abort = true;
        }

        // Deferred cross-element writes (Pascal pokes the target through a
        // live pointer mid-parse, e.g. RegControl `TapNum` → the transformer's
        // PresentTap; nothing reads the target in between, so applying after
        // the edit is equivalent). Collected before the flag propagation so
        // the active-class borrows can end before `classes` is re-borrowed.
        let ref_actions = objects[oi].take_ref_actions();

        // Signal-flag propagation (Pascal `Set_Bus`/`Set_Enabled` write the
        // circuit globals immediately; `Set_YprimInvalid` raises
        // `SystemYChanged` for enabled elements).
        if let Some(ckt) = circuit.as_mut()
            && let Some(elem) = objects[oi].as_ckt_element_mut()
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
            let tgt = &mut classes[target.cls].objects[target.idx];
            // `SetSwitchClosed`/`SetConductorsClosed` act on the generic
            // CktElement base (any switched element), so they are applied here
            // rather than through the per-class `apply_ref_action`; the
            // transformer-tap variant stays class-specific.
            match action {
                crate::obj::base::RefAction::SetSwitchClosed {
                    terminal, closed, ..
                } => {
                    if let Some(elem) = tgt.as_ckt_element_mut() {
                        elem.cd_mut().set_terminal_closed(*terminal, *closed);
                    }
                }
                crate::obj::base::RefAction::SetConductorsClosed {
                    terminal, closed, ..
                } => {
                    if let Some(elem) = tgt.as_ckt_element_mut() {
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
                    if let Some(elem) = tgt.as_ckt_element_mut() {
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
                    if let Some(elem) = tgt.as_ckt_element_mut() {
                        let cd = elem.cd_mut();
                        cd.flags
                            .include(crate::elements::ckt::ElemFlags::HAS_OCP_DEVICE);
                        if *auto {
                            cd.flags
                                .include(crate::elements::ckt::ElemFlags::HAS_AUTO_OCP_DEVICE);
                        }
                        if cd.ocp_device_type == 0 {
                            cd.ocp_device_type = *device_type;
                        }
                    }
                }
                _ => tgt.apply_ref_action(action),
            }
            // Propagate the target's flags too (a tap change invalidates the
            // transformer's Yprim exactly like a direct `Tap=` edit).
            if let Some(ckt) = circuit.as_mut()
                && let Some(elem) = tgt.as_ckt_element_mut()
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
}
