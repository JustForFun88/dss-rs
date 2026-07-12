//! Solution-driving and circuit-maintenance commands (`Solve`, `Sample`,
//! `Reset`, `AllocateLoads`, `RelCalc`, `Reduce`, `CalcVoltageBases`,
//! `BuildY`, `BusCoords`, `Redirect`/`Compile`). Split out of `exec/mod.rs`.

use super::*;

impl Dss {
    /// Pascal `DoSolveCmd`: `ActiveCircuit.Solution.Solve()`.
    pub(super) fn do_solve_cmd(&mut self) {
        // Pascal `TSolutionObj.Solve`'s `AUTOADDFLAG` arm dispatches to
        // `ckt.AutoAddObj.Solve()`, which re-enters the executive to add the
        // winner — impossible from the `(ckt, env)`-scoped solution dispatcher,
        // so it is intercepted here at the executive layer (exec/auto_add.rs).
        if self
            .circuit
            .as_ref()
            .expect("gated in command()")
            .solution
            .mode
            == SolveMode::AutoAdd
        {
            self.do_auto_add_solve();
            return;
        }
        // A-Diakoptics coordinator solve (Pascal `Solve` → `SolveSnap` with the
        // AD branch). The per-iteration stitch is WP-AD.3 Stage 2b.
        if self
            .circuit
            .as_ref()
            .expect("gated in command()")
            .solution
            .adiakoptics
        {
            self.ad_solve();
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

    /// Pascal `TExecHelper.DoCloseDICmd` (`ExecHelper.pas:4199`):
    /// `EnergyMeterClass.CloseAllDIFiles` — flush + close every open
    /// demand-interval file.
    pub(super) fn do_close_di_cmd(&mut self) {
        let Dss {
            classes,
            circuit,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        let mut store = ClassStore { classes };
        crate::solution::meters::close_all_di_files(ckt, &mut store, errors);
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
            output_directory,
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
            crate::solution::meters::reset_all_meters(ckt, env.store, output_directory, env.errors);
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
    /// (`DoReduceDefault`/`DoReduceShortLines`/…) → `TLineObj.MergeWith`
    /// (WP8.7, `exec/reduce.rs`).
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
            // All meters → ReduceZone on each.
            let meters = self
                .circuit
                .as_ref()
                .expect("gated in command()")
                .energy_meters
                .clone();
            for r in meters {
                self.reduce_zone(r);
            }
            return;
        }

        // Named meter: resolve it (Pascal `MeterClass.SetActive(Param)`); a
        // miss is error 262.
        let found = {
            let Dss {
                classes, circuit, ..
            } = self;
            let ckt = circuit.as_ref().expect("gated in command()");
            let store = ClassStore { classes };
            ckt.energy_meters
                .iter()
                .find(|&&r| {
                    store
                        .ckt_elem(r)
                        .cd()
                        .obj
                        .name()
                        .eq_ignore_ascii_case(&param)
                })
                .copied()
        };
        if let Some(r) = found {
            self.reduce_zone(r);
        } else {
            // Pascal error 262 (echoes the uppercased name).
            self.errors
                .push(format!("EnergyMeter \"{param}\" not found."));
        }
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
    /// `swap_xy` is the `LatLongCoords` variant (wired at WPG.16).
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

    /// Pascal `ExecCommands.pas` `ord(Cmd.MakeBusList)`: `with ActiveCircuit do
    /// if BusNameRedefined then ReprocessBusDefs` — nothing else.
    pub(super) fn do_make_bus_list_cmd(&mut self) {
        let Dss {
            classes,
            circuit,
            aux_parser,
            vars,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        if ckt.bus_name_redefined {
            let mut store = ClassStore { classes };
            ckt.reprocess_bus_defs(&mut store, aux_parser, vars, errors);
        }
    }

    /// Pascal `DoSetBusXYCmd` (`ExecHelper.pas:4716`): `SetBusXY bus=… x=… y=…`.
    /// Ported loop-for-loop: the bus lookup + coordinate write happens after
    /// **every** parameter (so a positional `SetBusXY b1 10 20` writes the bus
    /// three times, the last with the full X/Y), and an unknown bus logs error
    /// 28722 once per remaining parameter, exactly like the Pascal loop.
    pub(super) fn do_set_bus_xy_cmd(&mut self) {
        // Pascal `SetBusXYCommands := TCommandList.Create(['Bus', 'x', 'y'])`.
        let commands = CommandList::new(["Bus", "x", "y"]);
        let mut param_name = self.parser.next_param(&self.vars);
        let mut param = self.parser.make_string(&self.vars);
        let mut param_pointer = 0usize;
        let mut bus_name = String::new();
        let mut xval = 0.0;
        let mut yval = 0.0;
        while !param.is_empty() {
            if param_name.is_empty() {
                param_pointer += 1;
            } else {
                param_pointer = commands.get_command(&param_name).map_or(0, |i| i + 1);
            }
            match param_pointer {
                1 => bus_name = param.clone(),
                // Pascal `DblValue`; a malformed number parses as 0 through the
                // same `make_double` the parser uses elsewhere.
                2 => xval = self.parser.make_double(&self.vars).unwrap_or(0.0),
                3 => yval = self.parser.make_double(&self.vars).unwrap_or(0.0),
                _ => self
                    .errors
                    .push(format!("Error: Unknown Parameter on command line: {param}")),
            }
            let ckt = self.circuit.as_mut().expect("gated in command()");
            match ckt.bus_list.find(&bus_name) {
                Some(ib) => {
                    let bus = &mut ckt.buses[ib];
                    bus.x = xval;
                    bus.y = yval;
                    bus.coord_defined = true;
                }
                // Pascal error 28722.
                None => self
                    .errors
                    .push(format!("Error: Bus \"{bus_name}\" not found.")),
            }
            param_name = self.parser.next_param(&self.vars);
            param = self.parser.make_string(&self.vars);
        }
    }

    /// Pascal `DoSetkVBase` (`ExecHelper.pas:1949`): `SetkVBase bus=<name>
    /// kVLL=<v> | kVLN=<v>` — set one bus's `kVBase`. The value is taken L-N:
    /// a `kvln`-named second parameter is stored as-is, anything else (`kvll`
    /// or positional) is divided by √3. A hit raises
    /// `Solution.VoltageBaseChanged`; a miss appends `Bus <name> not found.`
    /// to `GlobalResult` (no error). Pascal also sets `ActiveBusIndex` — an
    /// inert side effect here (no ported command consumes an active bus; same
    /// class as `DoOpenCmd`'s `SetActiveBus`).
    pub(super) fn do_set_kv_base_cmd(&mut self) {
        self.parser.next_param(&self.vars);
        let bus_name = self.parser.make_string(&self.vars).to_lowercase();
        let param_name = self.parser.next_param(&self.vars).to_lowercase();
        let kv_value = self.parser.make_double(&self.vars).unwrap_or(0.0);

        let ckt = self.circuit.as_mut().expect("gated in command()");
        match ckt.bus_list.find(&bus_name) {
            Some(ib) => {
                ckt.buses[ib].kv_base = if param_name == "kvln" {
                    kv_value
                } else {
                    kv_value / crate::util::sqrt3()
                };
                ckt.solution.voltage_base_changed = true;
            }
            None => {
                super::helpers::append_result(
                    &mut self.last_result,
                    &format!("Bus {bus_name} not found."),
                );
            }
        }
    }

    /// Pascal `DolossesCmd` (`ExecHelper.pas:2168`): the active circuit
    /// element's `Losses` (kW/kvar) into `GlobalResult`,
    /// `Format('%10.5g, %10.5g')`. No active element → `GlobalResult`
    /// untouched (the no-circuit arm is dead — the dispatch gate errors #301
    /// pre-circuit).
    pub(super) fn do_losses_cmd(&mut self) {
        let Some((ci, oi)) = self.active_ckt_element else {
            return;
        };
        let Dss {
            classes,
            circuit,
            last_result,
            ..
        } = self;
        let ckt = circuit.as_ref().expect("gated in command()");
        let sys = crate::solution::solution::sys_ctx(ckt);
        let node_v = &ckt.solution.node_v;
        let Some(elem) = classes[ci].objects[oi].as_ckt_element_mut() else {
            return;
        };
        let loss = elem.losses(&sys, node_v);
        *last_result = format!(
            "{}, {}",
            crate::report::format::g_w(loss.re * 0.001, 10, 5),
            crate::report::format::g_w(loss.im * 0.001, 10, 5)
        );
    }

    /// Pascal `DoSummaryCmd` (`ExecHelper.pas:3449`): the solution summary into
    /// `GlobalResult` — status/mode/counts/iteration block, then the circuit
    /// summary (pu-voltage extremes, total source MW/Mvar, losses, frequency,
    /// mode/control-mode/load-model strings). Formats reproduced line-for-line
    /// (note the missing space in `Control Mode =%s` on the first occurrence,
    /// the trailing spaces after `Year/Hour/…voltage` values, and the literal
    /// `(**** %%)` in the zero-power losses arm — Pascal string concat, not a
    /// Format specifier).
    pub(super) fn do_summary_cmd(&mut self) {
        use crate::report::format;
        // The &mut element walks first (total source power + losses).
        let (tp_re_kw, tp_im_kvar) = self.total_power(); // Σ source power[1], kW
        let (loss_re_w, loss_im_var) = self.losses(); // W/var
        // `GetTotalPowerFromSources` = −Σ source power (VA); ×1e-6 → MVA.
        let c_power = (-tp_re_kw * 0.001, -tp_im_kvar * 0.001);
        let c_losses = (loss_re_w * 1e-6, loss_im_var * 1e-6);

        let ckt = self.circuit.as_ref().expect("gated in command()");
        let mode_str = self
            .enums
            .get(self.enums.solve_mode)
            .ordinal_to_string(ckt.solution.mode.ordinal());
        let control_str = self
            .enums
            .get(self.enums.control_mode)
            .ordinal_to_string(ckt.solution.control_mode);
        let load_model_str = self
            .enums
            .get(self.enums.default_load_model)
            .ordinal_to_string(ckt.solution.load_model);

        let mut s = String::new();
        s.push_str(if ckt.is_solved {
            "Status = SOLVED\n"
        } else {
            "Status = NOT Solved\n"
        });
        s.push_str(&format!("Solution Mode = {mode_str}\n"));
        s.push_str(&format!("Number = {}\n", ckt.solution.number_of_times));
        s.push_str(&format!(
            "Load Mult = {}\n",
            format::fixed_w(ckt.load_multiplier, 5, 3)
        ));
        s.push_str(&format!("Devices = {}\n", ckt.num_devices));
        s.push_str(&format!("Buses = {}\n", ckt.buses.len()));
        s.push_str(&format!("Nodes = {}\n", ckt.num_nodes));
        s.push_str(&format!("Control Mode ={control_str}\n"));
        s.push_str(&format!("Total Iterations = {}\n", ckt.solution.iteration));
        s.push_str(&format!(
            "Control Iterations = {}\n",
            ckt.solution.control_iteration
        ));
        s.push_str(&format!(
            "Max Sol Iter = {}\n",
            ckt.solution.most_iterations_done
        ));
        s.push_str(" \n - Circuit Summary -\n \n");
        s.push_str(&format!("Year = {} \n", ckt.solution.year));
        s.push_str(&format!("Hour = {} \n", ckt.solution.int_hour));
        s.push_str(&format!(
            "Max pu. voltage = {} \n",
            format::g(crate::report::export::max_pu_voltage(ckt), 5)
        ));
        s.push_str(&format!(
            "Min pu. voltage = {} \n",
            format::g(crate::report::export::min_pu_voltage(ckt, true), 5)
        ));
        s.push_str(&format!(
            "Total Active Power:   {} MW\n",
            format::g(c_power.0, 6)
        ));
        s.push_str(&format!(
            "Total Reactive Power: {} Mvar\n",
            format::g(c_power.1, 6)
        ));
        if c_power.0 != 0.0 {
            s.push_str(&format!(
                "Total Active Losses:   {} MW, ({} %)\n",
                format::g(c_losses.0, 6),
                format::g(c_losses.0 / c_power.0 * 100.0, 4)
            ));
        } else {
            s.push_str("Total Active Losses:   ****** MW, (**** %%)\n");
        }
        s.push_str(&format!(
            "Total Reactive Losses: {} Mvar\n",
            format::g(c_losses.1, 6)
        ));
        s.push_str(&format!(
            "Frequency = {} Hz\n",
            format::g(ckt.solution.frequency, 15)
        ));
        s.push_str(&format!("Mode = {mode_str}\n"));
        s.push_str(&format!("Control Mode = {control_str}\n"));
        s.push_str(&format!("Load Model = {load_model_str}\n"));
        self.last_result = s;
    }

    /// The step-solution commands (`ExecCommands.pas:578-601`): thin drivers
    /// over the ported solution internals — `_InitSnap` (`SnapShotInit`),
    /// `_SolveNoControl` (`SolveCircuit`), `_SampleControls`
    /// (`SampleControlDevices`), `_DoControlActions` (`DoControlActions`),
    /// `_ShowControlQueue` (the same `WriteQueue` CSV the `Show controlqueue`
    /// arm emits, `FireOffEditor` dropped per the GUI no-op rule),
    /// `_SolveDirect` (`SolveDirect`), `_SolvePFlow` (`DoPFLOWsolution`).
    pub(super) fn do_step_solution_cmd(&mut self, pointer: usize) {
        if pointer == cmd::SHOW_CONTROL_QUEUE {
            let ckt = self.circuit.as_ref().expect("gated in command()");
            let content = crate::report::show::show_control_queue(&self.classes, ckt);
            self.write_show_named("ControlQueue.csv", &content, false);
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
        let ckt = circuit.as_mut().expect("gated in command()");
        if pointer == cmd::INIT_SNAP {
            // Pascal `TSolutionObj.SnapShotInit` = SetGeneratorDispRef + the
            // counter/flag reset (split across two fns in this port).
            crate::solution::solution::set_generator_disp_ref(ckt);
            ckt.solution.snap_shot_init();
            return;
        }
        let mut store = ClassStore { classes };
        let mut env = SolveEnv {
            store: &mut store,
            parser: aux_parser,
            vars,
            errors,
        };
        // Hard errors are recorded by the solve internals (the `do_solve_cmd`
        // convention).
        let _ = match pointer {
            cmd::SOLVE_NO_CONTROL => crate::solution::solution::solve_circuit(ckt, &mut env),
            cmd::SAMPLE_CONTROLS => {
                crate::solution::controls::sample_control_devices(ckt, &mut env)
            }
            cmd::DO_CONTROL_ACTIONS => crate::solution::controls::do_control_actions(ckt, &mut env),
            cmd::SOLVE_DIRECT => crate::solution::solution::solve_direct(ckt, &mut env),
            cmd::SOLVE_PFLOW => crate::solution::solution::do_pflow_solution(ckt, &mut env),
            _ => unreachable!("dispatch covers the step-solution ordinals"),
        };
    }

    /// Pascal `DoInterpolateCmd` (`ExecHelper.pas:3106`): interpolate bus
    /// coordinates in meter zones. Clears `Flg.Checked` on every circuit
    /// element, then runs `InterpolateCoordinates` on every enabled meter
    /// (empty param → `'A'`) or on the named meter (disabled → error 283,
    /// missing → error 277).
    pub(super) fn do_interpolate_cmd(&mut self) {
        self.parser.next_param(&self.vars);
        let mut param = self.parser.make_string(&self.vars).to_uppercase();

        let meter_ci = self.class_by_name.get("energymeter").copied();

        // Resolve the named meter before splitting the borrows (Pascal
        // `MeterClass.SetActive(Param)` — sets the class's active object).
        if param.is_empty() {
            param = "A".to_string();
        }
        let named: Option<Result<usize, ()>> = if param.starts_with('A') {
            None
        } else {
            let Some(ci) = meter_ci else {
                return; // `ClassNames.Find('energymeter')` cannot miss here
            };
            if self.classes[ci].set_active(&param) {
                Some(Ok(self.classes[ci].active.expect("set_active sets it")))
            } else {
                Some(Err(()))
            }
        };

        let Dss {
            classes,
            circuit,
            errors,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        let mut store = ClassStore { classes };

        // Initialize the Checked flag for all circuit elements.
        let refs: Vec<ElemRef> = ckt.ckt_elements.clone();
        for r in refs {
            store
                .ckt_elem_mut(r)
                .cd_mut()
                .flags
                .exclude(crate::elements::ckt::ElemFlags::CHECKED);
        }

        match named {
            None => {
                // 'A': every enabled meter, circuit meter-list order.
                let meters: Vec<ElemRef> = ckt.energy_meters.clone();
                for r in meters {
                    if store.ckt_elem(r).cd().enabled {
                        crate::solution::meters::interpolate_coordinates(
                            r, ckt, &mut store, errors,
                        );
                    }
                }
            }
            Some(Ok(idx)) => {
                let ci = meter_ci.expect("named branch requires the class");
                let r = ElemRef { cls: ci, idx };
                if store.ckt_elem(r).cd().enabled {
                    crate::solution::meters::interpolate_coordinates(r, ckt, &mut store, errors);
                } else {
                    // Pascal error 283 (Param is the uppercased name).
                    errors.push(format!("EnergyMeter \"{param}\" is disabled."));
                }
            }
            // Pascal error 277.
            Some(Err(())) => errors.push(format!("EnergyMeter \"{param}\" not found.")),
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
        // Pascal `DoRedirect` loads the deck with `TStringList.LoadFromFile`
        // (ExecHelper.pas:433), whose UTF-8 stream reader strips a leading
        // byte-order mark (EF BB BF) before the first line. Without this the BOM
        // glues onto the first token ("Unknown Command \u{feff}Clear") and the deck
        // builds a subtly wrong circuit. Every Redirect/Compile file passes through
        // here, so nested redirects are covered too.
        let content = content.strip_prefix('\u{feff}').unwrap_or(&content);

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
