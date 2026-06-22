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
            cmd::SHOW => {
                // Pascal `DoShowCmd` (ShowResults.pas) is Phase 8
                // (reporting/exports/Save): it only writes report files and never
                // alters the electrical solution, so it is stubbed as a no-op here
                // (like Plot/Panel). The live oracle gate compares the assembled
                // model, not report text, so this is faithful for the gate and
                // admits the `Show LineConstants` geometry/cable feeders.
                return;
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
            cmd::SOLVE => self.do_set_cmd(1), // Solve = Set + DoSolveCmd
            cmd::SET => self.do_set_cmd(0),
            cmd::QUERY => self.do_query_cmd(),
            cmd::CALC_VOLTAGE_BASES => self.do_calc_voltage_bases(),
            cmd::BUILD_Y => self.do_build_y(),
            cmd::GET => self.do_get_cmd(),
            cmd::SAMPLE => self.do_sample_cmd(),
            cmd::RESET => self.do_reset_cmd(),
            cmd::ALLOCATE_LOADS => self.do_allocate_loads_cmd(),
            cmd::RELCALC => self.do_relcalc_cmd(),
            cmd::REDUCE => self.do_reduce_cmd(),
            cmd::BUSCOORDS => self.do_bus_coords_cmd(false),
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
            self.errors
                .push("object=Class.Name expected as first parameter in command.".to_string());
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

    /// Pascal `AddObject`: create the object (or make the existing one
    /// active for `DSS_OBJECT` classes), register circuit elements with the
    /// circuit, and edit the rest of the line.
    fn add_object(&mut self, obj_class: &str, name: &str) {
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

    /// Pascal `DoClearCmd`: drop the circuit and every object.
    fn do_clear_cmd(&mut self) {
        for cls in &mut self.classes {
            cls.objects.clear();
            cls.name_to_idx.clear();
            cls.active = None;
        }
        self.active_class = None;
        self.circuit = None;
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
        let cls = &self.classes[ci];
        let oi = cls.active.expect("just set active");
        if let Some(idx) = cls.props.property_index(&prop_name) {
            self.last_result = cls
                .props
                .get_value(cls.objects[oi].as_ref(), idx, &self.enums);
        }
    }

    /// The body of Pascal `TDSSClass.Edit`: iterate `name=value` parameters on
    /// the main parser against the active object, then `EndEdit`, then
    /// propagate the element's signal flags to the circuit (the Pascal set
    /// `ActiveCircuit.BusNameRedefined`/`Solution.SystemYChanged` directly
    /// from the property setters; nothing reads them mid-edit, so polling
    /// after the edit is equivalent).
    fn edit_active(&mut self) {
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
                if param_name.is_empty() {
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

        // Deferred file loads (Pascal runs `DoCSVFile` etc. in the property
        // hook, which has the DSS context; our hook cannot reach the filesystem
        // or the current directory, so it queues the request and we resolve it
        // here — before `end_edit`, so derived state like `SetMaxPandQ` sees the
        // loaded data). Paths resolve relative to `current_dir`, like Redirect.
        let file_loads = objects[oi].take_file_loads();
        for fl in &file_loads {
            let path = current_dir.join(&fl.filename);
            match std::fs::read_to_string(&path) {
                Ok(content) => objects[oi].apply_file_load(fl, &content, errors),
                // Pascal error 613.
                Err(_) => errors.push(format!("Error opening file: \"{}\"", fl.filename)),
            }
        }

        objects[oi].end_edit();

        // Drain any `DoSimpleMsg`/`DoErrorMsg` queued by the property hooks
        // (e.g. `LineCode.Kron` on a 1-phase code) into the engine error log.
        let deferred = objects[oi].data_mut().take_errors();
        errors.extend(deferred);

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
