//! Solution-driving and circuit-maintenance commands (`Solve`, `Sample`,
//! `Reset`, `AllocateLoads`, `RelCalc`, `Reduce`, `CalcVoltageBases`,
//! `BuildY`, `BusCoords`, `Redirect`/`Compile`). Split out of `exec/mod.rs`.

use super::*;

impl Dss {
    /// Pascal `DoSolveCmd`: `ActiveCircuit.Solution.Solve()`.
    pub(super) fn do_solve_cmd(&mut self) {
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        let _ = solve(ckt, &mut env); // hard errors are recorded by solve()
    }

    /// Pascal `DoSampleCmd` (`ExecHelper.pas` l.1036): `MonitorClass.SampleAll`
    /// — force every enabled monitor (mode ≠ 5) to take a sample.
    pub(super) fn do_sample_cmd(&mut self) {
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        let sys = crate::solution::solution::sys_ctx(ckt);
        crate::solution::monitors::sample_all_monitors(ckt, &mut env, false);
        // Pascal `DoSampleCmd` l.1037: `EnergyMeterClass.SampleAll` (gets
        // generators too — the generator register sweep is WP6.8).
        crate::solution::meters::take_sample_all(ckt, env.store, &sys);
    }

    /// Pascal `TExecHelper.DoAllocateLoadsCmd` (`ExecHelper.pas` l.2605): adjust
    /// loads defined by connected kVA or kWh billing to match the EnergyMeter /
    /// Sensor measured peaks. Solves a snapshot guess, then iterates
    /// `MaxAllocationIterations` times: recompute each meter/sensor allocation
    /// factor, run each meter's zone allocation, and re-solve.
    pub(super) fn do_allocate_loads_cmd(&mut self) {
        let max_iters = self.max_allocation_iterations.max(0) as usize;
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        ckt.load_multiplier = 1.0;
        // Pascal `DoAllocateLoadsCmd` (ExecHelper.pas l.2617): force SNAPSHOT
        // before the guess solve — `if Mode <> SNAPSHOT then Mode := SNAPSHOT`,
        // whose `Set_Mode` side effect re-inits the solution and clears the
        // meter-sampling state that a prior yearly/daily run may have left set.
        if ckt.solution.mode != SolveMode::Snapshot {
            crate::solution::set_mode(ckt, SolveMode::Snapshot, errors);
        }
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        crate::solution::meters::allocate_loads(ckt, &mut env, max_iters);
    }

    /// Pascal `DoResetCmd` (`ExecHelper.pas` l.1527): with no argument, reset
    /// monitors, meters, controls and clear the event/error logs; otherwise the
    /// first letter selects the target (`MOnitors`/`MEters`/`Controls`/
    /// `Eventlog`). Faults (`F`) and the topology `KeepList` (`K`) have no class
    /// in this port yet, so those selectors are accepted as no-ops.
    pub(super) fn do_reset_cmd(&mut self) {
        self.parser.next_param(&self.vars);
        let param = self.parser.make_string(&self.vars).to_uppercase();
        let b = param.as_bytes();
        // Decode the Pascal `case Param[1] of` dispatch into a set of targets.
        let (do_monitors, do_meters, do_faults, do_controls, do_eventlog) = if param.is_empty() {
            (true, true, true, true, true)
        } else {
            match b.first() {
                Some(&b'M') => (
                    b.get(1) == Some(&b'O'),
                    b.get(1) == Some(&b'E'),
                    false,
                    false,
                    false,
                ),
                Some(&b'F') => (false, false, true, false, false),
                Some(&b'C') => (false, false, false, true, false),
                Some(&b'E') => (false, false, false, false, true),
                // `K` (keep list) is a later-phase class; accept the selector
                // without erroring so scripts don't abort.
                Some(&b'K') => (false, false, false, false, false),
                _ => {
                    self.errors
                        .push(format!("Unknown argument to Reset Command: \"{param}\""));
                    return;
                }
            }
        };
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        if do_monitors {
            crate::solution::monitors::reset_all_monitors(ckt, &mut env);
        }
        if do_meters {
            crate::solution::meters::reset_all_meters(ckt, env.store);
        }
        if do_faults {
            // Pascal `DoResetFaults`: `Reset()` on every Fault.
            crate::solution::faults::reset_faults(ckt, &mut env);
        }
        if do_controls {
            // Pascal `DoResetControls`: `Reset()` on every enabled control.
            let _ = crate::solution::controls::reset_all_controls(ckt, &mut env);
        }
        if do_eventlog {
            ckt.solution.event_log.clear();
        }
    }

    /// Pascal `TExecHelper.DoLambdaCalcs` (the `RelCalc` command): fault-rate and
    /// bus-interruption reliability calc over every EnergyMeter zone. The single
    /// positional parameter (any name) is the `AssumeRestoration` yes/no flag.
    pub(super) fn do_relcalc_cmd(&mut self) {
        // EnergyMeter objects required (Pascal error 28724).
        if self
            .circuit
            .as_ref()
            .expect("gated in command()")
            .energy_meters
            .is_empty()
        {
            self.errors.push(
                "No EnergyMeter Objects Defined. EnergyMeter objects required for this function."
                    .to_string(),
            );
            return;
        }
        self.parser.next_param(&self.vars);
        let param = self.parser.make_string(&self.vars);
        let assume_restoration = !param.is_empty() && interpret_yes_no(&param);

        let Dss {
            classes,
            circuit,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        let mut store = ClassStore { classes };
        let errs = crate::solution::meters::calc_all_reliability_indices(
            ckt,
            &mut store,
            assume_restoration,
        );
        errors.extend(errs);
    }

    /// Pascal `TExecHelper.MarkCapandReactorBuses` (ExecHelper.pas l.1573): mark
    /// every bus carrying an *enabled, shunt-connected* capacitor or reactor as
    /// a "keeper" (`Bus.Keep := TRUE`) so a later circuit reduction won't
    /// eliminate it. Runs as a side-effect of the `Reduce` command regardless of
    /// whether the reduction itself proceeds.
    fn mark_cap_and_reactor_buses(&mut self) {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        // `ElemRef` is `Copy`; snapshot the refs so the bus write below doesn't
        // alias the element-list borrow (the store borrows `classes`, disjoint
        // from `ckt`).
        let refs: Vec<ElemRef> = ckt
            .shunt_capacitors
            .iter()
            .chain(ckt.reactors.iter())
            .copied()
            .collect();
        let store = ClassStore { classes };
        for r in refs {
            let elem = store.ckt_elem(r);
            if elem.is_shunt() && elem.cd().enabled {
                let bus = elem.cd().terminals[0].bus_ref;
                if let Some(b) = ckt.buses.get_mut(bus) {
                    b.keep = true;
                }
            }
        }
    }

    /// Pascal `DoReduceCmd` (ExecHelper.pas l.1614): the `Reduce` command. The
    /// observable surface is reproduced faithfully — the cap/reactor bus marking
    /// ([`Self::mark_cap_and_reactor_buses`]), the error-1890 no-meter
    /// precondition, the `'A'`(ll)-vs-named-meter dispatch, and the error-262
    /// "EnergyMeter not found". The reduction *work itself* —
    /// `EnergyMeter.ReduceZone` dispatching `ReduceAlgs.pas`
    /// (`DoReduceDefault`/`DoReduceShortLines`/…) → `TLineObj.MergeWith` — is
    /// NOT_PORTED (the 210-line line merge is unported), so a resolved meter
    /// records a deferral instead of reducing its zone.
    pub(super) fn do_reduce_cmd(&mut self) {
        // Pascal reads the next parm and uppercases it (`AnsiUpperCase`).
        self.parser.next_param(&self.vars);
        let mut param = self.parser.make_string(&self.vars).to_uppercase();

        // Pascal marks cap/reactor buses Keep *before* the meter-count check.
        self.mark_cap_and_reactor_buses();

        let no_meters = self
            .circuit
            .as_ref()
            .expect("gated in command()")
            .energy_meters
            .is_empty();
        if no_meters {
            // Pascal error 1890.
            self.errors.push(
                "An energy meter is required to use this feature. Please check \
                 https://sourceforge.net/p/electricdss/code/HEAD/tree/trunk/Version8/Doc/Circuit%20Reduction%20for%20Version8.docx \
                 for examples."
                    .to_string(),
            );
            return;
        }

        // Pascal: empty arg defaults to 'A' (all meters).
        if param.is_empty() {
            param = "A".to_string();
        }

        if param.starts_with('A') {
            // All meters → ReduceZone on each (NOT_PORTED).
            self.errors.push(Self::reduce_deferred_msg());
            return;
        }

        // Named meter: resolve it (Pascal `MeterClass.SetActive(Param)`); a
        // miss is error 262, a hit would `ReduceZone` (NOT_PORTED → deferral).
        let found = {
            let Dss {
                classes, circuit, ..
            } = self;
            let ckt = circuit.as_ref().expect("gated in command()");
            let store = ClassStore { classes };
            ckt.energy_meters.iter().any(|&r| {
                store
                    .ckt_elem(r)
                    .cd()
                    .obj
                    .name()
                    .eq_ignore_ascii_case(&param)
            })
        };
        if found {
            self.errors.push(Self::reduce_deferred_msg());
        } else {
            // Pascal error 262 (echoes the uppercased name).
            self.errors
                .push(format!("EnergyMeter \"{param}\" not found."));
        }
    }

    /// The NOT_PORTED deferral logged when a `Reduce` would otherwise call
    /// `EnergyMeter.ReduceZone` (see [`Self::do_reduce_cmd`]).
    fn reduce_deferred_msg() -> String {
        "Reduce: circuit reduction is not ported yet (the zone line-merge \
         requires Line.MergeWith — deferred to a later phase)."
            .to_string()
    }

    /// Pascal `DoSetVoltageBases` (the `CalcVoltageBases` command).
    pub(super) fn do_calc_voltage_bases(&mut self) {
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        if let Err(e) = set_voltage_bases(ckt, &mut env) {
            env.errors
                .push(format!("Error Encountered in CalcVoltageBases: {e}"));
        }
    }

    /// Pascal `InvalidateAllPCElements` (the `BuildY` command).
    pub(super) fn do_build_y(&mut self) {
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        for &r in &ckt.pc_elements {
            if let Some(elem) = classes[r.cls].objects[r.idx].as_ckt_element_mut() {
                let cd = elem.cd_mut();
                cd.yprim_invalid = true;
                if cd.enabled {
                    ckt.solution.system_y_changed = true;
                }
            }
        }
    }

    /// Pascal `DoBusCoordsCmd` (`ExecHelper.pas` l.2955): read a `bus, x, y`
    /// file (one bus per line, aux-parser delimiters) and set the coordinates
    /// on buses that exist; buses not in the circuit are silently ignored.
    /// `swap_xy` is the `LatLongCoords` variant (unported command).
    pub(super) fn do_bus_coords_cmd(&mut self, swap_xy: bool) {
        self.parser.next_param(&self.vars);
        let param = self.parser.make_string(&self.vars);
        let path = self.current_dir.join(&param);
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                self.errors.push(format!(
                    "Bus Coordinate file \"{param}\" could not be read: {e}"
                ));
                return;
            }
        };
        let Dss {
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        for (lineno, line) in content.lines().enumerate() {
            aux_parser.set_cmd_string(line);
            aux_parser.next_param(vars);
            let bus_name = aux_parser.make_string(vars);
            let Some(ib) = ckt.bus_list.find(&bus_name) else {
                continue; // just ignore a bus that's not in the circuit
            };
            // Pascal reads both coordinates with DblValue; a malformed number
            // raises and aborts the whole file with error 275.
            aux_parser.next_param(vars);
            let first = aux_parser.make_double(vars);
            aux_parser.next_param(vars);
            let second = aux_parser.make_double(vars);
            let (Ok(first), Ok(second)) = (first, second) else {
                errors.push(format!(
                    "Bus Coordinate file: Error Reading Line {}",
                    lineno + 1
                ));
                return;
            };
            let bus = &mut ckt.buses[ib];
            if swap_xy {
                bus.y = first;
                bus.x = second;
            } else {
                bus.x = first;
                bus.y = second;
            }
            bus.coord_defined = true;
        }
    }

    /// Pascal `DoRedirect` (`Redirect`/`Compile`): run a script file line by
    /// line, handling `/* ... */` block comments exactly like the original
    /// (`/*` only recognized at the start of a line; `*/` anywhere in one).
    pub(super) fn do_redirect(&mut self, is_compile: bool) {
        self.parser.next_param(&self.vars);
        let fname = self.parser.make_string(&self.vars);
        if fname.is_empty() {
            return; // ignore altogether if null filename
        }

        // Expand relative to the current DSS directory.
        let mut path = self.current_dir.join(&fname);
        if !path.is_file() {
            // Try appending '.dss' if there is no extension yet.
            if !fname.contains('.') {
                path = self.current_dir.join(format!("{fname}.dss"));
            }
            if !path.is_file() {
                self.errors
                    .push(format!("Redirect file not found: \"{fname}\""));
                if let Some(ckt) = self.circuit.as_mut() {
                    ckt.solution.solution_abort = true;
                }
                return;
            }
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                self.errors
                    .push(format!("Redirect File \"{fname}\" could not be read: {e}"));
                if let Some(ckt) = self.circuit.as_mut() {
                    ckt.solution.solution_abort = true;
                }
                return;
            }
        };

        // Change directory to the file's path in case it loads more files.
        let save_dir = self.current_dir.clone();
        let curr_dir = path.parent().map(|p| p.to_path_buf());
        if let Some(d) = &curr_dir {
            self.current_dir = d.clone();
            // Pascal `if IsCompile then SetDataPath(DSS, CurrDir)`
            // (`ExecHelper.pas:546`): Compile also moves the report
            // OutputDirectory to the deck's directory, so exports issued
            // *inside* the compiled script already land next to the deck
            // (oracle-verified). Redirect moves only the current dir.
            if is_compile {
                self.output_directory = d.clone();
            }
        }

        self.redirect_abort = false;
        let save_in_redirect = self.in_redirect; // nested Redirects stay "inside"
        self.in_redirect = true;

        let mut in_block_comment = false;
        let lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        for input_line in &lines {
            if self.redirect_abort {
                break;
            }
            if input_line.is_empty() {
                continue;
            }
            if !in_block_comment && input_line.starts_with("/*") {
                in_block_comment = true;
            }
            if !in_block_comment {
                let solution_abort = self
                    .circuit
                    .as_ref()
                    .is_some_and(|c| c.solution.solution_abort);
                if !solution_abort {
                    self.command(input_line);
                } else {
                    self.redirect_abort = true; // Abort file if solution was aborted
                }
            }
            if in_block_comment && input_line.contains("*/") {
                in_block_comment = false;
            }
        }

        self.in_redirect = save_in_redirect;
        let path_str = path.to_string_lossy().to_string();
        self.vars.add("@lastfile", &path_str);
        if is_compile {
            // Pascal re-runs `SetDataPath(DSS, CurrDir)` in the `finally`
            // (`ExecHelper.pas:651`): Compile keeps the script directory as the
            // data path *and* report OutputDirectory — re-asserted so a nested
            // Compile inside the script cannot leave them elsewhere.
            if let Some(d) = curr_dir {
                self.current_dir = d.clone();
                self.output_directory = d;
            }
            self.vars.add("@lastcompilefile", &path_str);
        } else {
            self.current_dir = save_dir; // Redirect returns to where we were
            self.vars.add("@lastredirectfile", &path_str);
        }
    }
}
