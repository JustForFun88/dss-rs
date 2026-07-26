//! Pascal `TAutoAdd.Solve` (`Common/AutoAdd.pas` l.267) — the AutoAdd
//! capacity-search solve driver, dispatched from `TSolutionObj.Solve`'s
//! `AUTOADDFLAG` arm. It lives at the executive layer (not in the solution
//! dispatcher) because the winner is instantiated by re-entering the executive
//! command path (`DSS.DSSExecutive.ParseCommand('New generator.Gadd…')`),
//! exactly like Pascal.
//!
//! The per-candidate current injection (`AddCurrents`, wired through
//! `Solution.AddInAuxCurrents` under `UseAuxCurrents`) is in
//! `solution/solution/power_flow.rs`; the record + `(ckt, store)` helpers
//! (`MakeBusList`, `ComputekWLosses_EEN`, `Get_WeightedLosses`) are in
//! `circuit/auto_add.rs`.

use std::path::Path;

use num_complex::Complex64;

use super::*;
use crate::circuit::AddType;
use crate::circuit::auto_add::{compute_kw_losses_een, make_bus_list, weighted_losses};
use crate::report::format::{fixed_w, g};
use crate::solution::meters::{reset_all_meters, take_sample_all};
use crate::solution::{ControlMode, LoadSolutionModel, solve_snap, sys_ctx};
use crate::util::sqrt3;

/// The result of the candidate-bus search (everything the executive needs to
/// instantiate the winner + set `GlobalResult`).
struct AutoAddOutcome {
    /// `AddType` (GENADD/CAPADD) the search ran.
    add_type: AddType,
    /// Winning bus as a Pascal 1-based `BusList` index (0 = none found).
    min_loss_bus: usize,
    /// The winner's phase count (1 or 3).
    min_bus_phases: i32,
    /// `MaxLossImproveFactor` — the winning weighted objective.
    max_loss_improve_factor: f64,
    /// `TestGenkW` (GENADD) — the per-search generator kW (possibly /3 for a
    /// positive-sequence circuit).
    test_gen_kw: f64,
    /// The (possibly-corrected) generator power factor used in the command.
    gen_pf: f64,
    /// `TestCapkvar` (CAPADD).
    test_cap_kvar: f64,
    /// The accumulated `…AutoAddLog.csv` contents.
    log_content: String,
}

/// Pascal `TAutoAdd.Solve` — the candidate search (steps 1-4), stopping just
/// before the winner is added. Runs entirely on `(ckt, env)`; the executive
/// caller performs the final `New generator/capacitor` add + re-solve (step 5).
fn auto_add_search(
    ckt: &mut Circuit,
    env: &mut SolveEnv,
    output_directory: &Path,
) -> Result<AutoAddOutcome, String> {
    if ckt.solution.load_model == LoadSolutionModel::Admittance {
        ckt.solution.load_model = LoadSolutionModel::PowerFlow;
        ckt.solution.system_y_changed = true; // Force rebuild of System Y without Loads
    }

    // Preliminary snapshot to force meter-zone definition + bus lists.
    reset_all_meters(ckt, env.store, output_directory, env.errors);
    if ckt.solution.system_y_changed || ckt.bus_name_redefined {
        solve_snap(ckt, env)?;
        ckt.auto_add_obj.mode_changed = true;
    }
    {
        let sys = sys_ctx(ckt);
        take_sample_all(ckt, env.store, &sys);
    }

    // Check that bus base voltages are defined (Pascal `ckt.Buses[NumBuses]`).
    if !ckt.buses.is_empty() && ckt.buses[ckt.buses.len() - 1].kv_base == 0.0 {
        set_voltage_bases(ckt, env)?;
    }

    if ckt.auto_add_obj.mode_changed {
        ckt.auto_add_obj.bus_idx_list = make_bus_list(ckt, env.store);
        ckt.auto_add_obj.mode_changed = false; // Keep same BusIdxList if no changes
    }

    ckt.solution.interval_hrs = 1.0;

    // Start the log file (Pascal FSWriteLn header). SetGeneratorDispRef is
    // redundant here — every `solve_snap` below re-establishes it — so it is
    // left to the per-candidate solves.
    let mut log_content = String::from(
        "\"Bus\", \"Base kV\", \"kW Losses\", \"% Improvement\", \"kW UE\", \"% Improvement\", \"Weighted Total\", \"Iterations\"\n",
    );

    // Turn regulators and caps off while searching.
    ckt.solution.control_mode = ControlMode::ControlsOff;

    // Establish base values (Pascal `SetBaseLosses`).
    let (base_losses, base_een) = {
        let sys = sys_ctx(ckt);
        compute_kw_losses_een(ckt, env.store, &sys)
    };

    let add_type = ckt.auto_add_obj.add_type;
    let mut min_loss_bus = 0usize; // Pascal 1-based; 0 = null string
    let mut max_loss_improve_factor = -1.0e50;
    let mut min_bus_phases = 3;
    let mut test_gen_kw = 0.0;
    let mut test_cap_kvar = 0.0;

    let bus_idx_list = ckt.auto_add_obj.bus_idx_list.clone();
    let gen_kw = ckt.auto_add_obj.gen_kw;

    match add_type {
        AddType::Gen => {
            test_gen_kw = if ckt.positive_sequence {
                ckt.auto_add_obj.gen_kw / 3.0
            } else {
                ckt.auto_add_obj.gen_kw
            };

            if ckt.auto_add_obj.gen_pf != 0.0 {
                let gen_pf = ckt.auto_add_obj.gen_pf;
                let mut genkvar = test_gen_kw * (1.0 / (gen_pf * gen_pf) - 1.0).sqrt();
                if gen_pf < 0.0 {
                    genkvar = -genkvar;
                }
                ckt.auto_add_obj.gen_kvar = genkvar;
            } else {
                // Someone goofed and specified 0.0 PF.
                ckt.auto_add_obj.gen_pf = 1.0;
                ckt.auto_add_obj.gen_kvar = 0.0;
            }

            for bus_index in bus_idx_list {
                if bus_index > 0 {
                    let rust_bus = bus_index - 1;
                    reset_all_meters(ckt, env.store, output_directory, env.errors);

                    // 3-phase or 1-phase generator, by nodes at the bus.
                    let phases = if ckt.buses[rust_bus].num_nodes_this_bus() < 3 {
                        1
                    } else {
                        3
                    };
                    let genkvar = ckt.auto_add_obj.gen_kvar;
                    let gen_va = Complex64::new(
                        1000.0 * test_gen_kw / phases as f64,
                        1000.0 * genkvar / phases as f64,
                    );

                    // Publish the trial-device state for `AddCurrents`.
                    ckt.auto_add_obj.bus_index = bus_index;
                    ckt.auto_add_obj.phases = phases;
                    ckt.auto_add_obj.gen_va = gen_va;

                    ckt.is_solved = false;
                    ckt.solution.use_aux_currents = true; // Calls InjCurrents on callback
                    solve_snap(ckt, env)?;

                    if ckt.is_solved {
                        // Only score a converged solution.
                        {
                            let sys = sys_ctx(ckt);
                            take_sample_all(ckt, env.store, &sys);
                        }
                        let lf = {
                            let sys = sys_ctx(ckt);
                            weighted_losses(ckt, env.store, &sys, base_losses, base_een, gen_kw)
                        };
                        append_log_row(&mut log_content, ckt, rust_bus, &lf);
                        if lf.weighted > max_loss_improve_factor {
                            max_loss_improve_factor = lf.weighted;
                            min_loss_bus = bus_index;
                            min_bus_phases = phases;
                        }
                    }
                }
                if ckt.solution.solution_abort {
                    break;
                }
            }
        }
        AddType::Cap => {
            test_cap_kvar = if ckt.positive_sequence {
                ckt.auto_add_obj.cap_kvar / 3.0
            } else {
                ckt.auto_add_obj.cap_kvar
            };

            for bus_index in bus_idx_list {
                if bus_index > 0 {
                    let rust_bus = bus_index - 1;
                    reset_all_meters(ckt, env.store, output_directory, env.errors);

                    let phases = if ckt.buses[rust_bus].num_nodes_this_bus() < 3 {
                        1
                    } else {
                        3
                    };
                    // Apply the capacitor at the bus L-N base kV.
                    let kvrat = ckt.buses[rust_bus].kv_base;
                    let ycap = (test_cap_kvar * 0.001 / phases as f64) / (kvrat * kvrat);

                    ckt.auto_add_obj.bus_index = bus_index;
                    ckt.auto_add_obj.phases = phases;
                    ckt.auto_add_obj.ycap = ycap;

                    ckt.is_solved = false;
                    ckt.solution.use_aux_currents = true;
                    solve_snap(ckt, env)?;

                    if ckt.is_solved {
                        {
                            let sys = sys_ctx(ckt);
                            take_sample_all(ckt, env.store, &sys);
                        }
                        let lf = {
                            let sys = sys_ctx(ckt);
                            weighted_losses(ckt, env.store, &sys, base_losses, base_een, gen_kw)
                        };
                        append_log_row(&mut log_content, ckt, rust_bus, &lf);
                        if lf.weighted > max_loss_improve_factor {
                            max_loss_improve_factor = lf.weighted;
                            min_loss_bus = bus_index;
                            min_bus_phases = phases;
                        }
                    }
                }
                if ckt.solution.solution_abort {
                    break;
                }
            }
        }
    }

    // Put control mode back to default before inserting the device for real.
    ckt.solution.control_mode = ControlMode::Static;
    ckt.solution.use_aux_currents = false;

    Ok(AutoAddOutcome {
        add_type,
        min_loss_bus,
        min_bus_phases,
        max_loss_improve_factor,
        test_gen_kw,
        gen_pf: ckt.auto_add_obj.gen_pf,
        test_cap_kvar,
        log_content,
    })
}

/// Pascal's per-candidate `FSWrite`/`FSWriteln` log line (`AutoAdd.pas`
/// l.429-432 / l.541-544): the tested bus, its L-L base kV, the losses/UE and
/// their percentage improvements, the weighted total, and the iteration count.
fn append_log_row(
    log: &mut String,
    ckt: &Circuit,
    rust_bus: usize,
    lf: &crate::circuit::auto_add::LossFigures,
) {
    let test_bus = ckt.bus_list.name(rust_bus).unwrap_or("");
    let base_kv = ckt.buses[rust_bus].kv_base * sqrt3();
    log.push_str(&format!(
        "\"{}\", {}, {}, {}, {}, {}, {}, {}\n",
        test_bus,
        g(base_kv, 15),
        g(lf.kw_losses, 15),
        g(lf.pu_loss_improvement * 100.0, 15),
        g(lf.kw_een, 15),
        g(lf.pu_een_improvement * 100.0, 15),
        g(lf.weighted, 15),
        ckt.solution.iteration,
    ));
}

impl Dss {
    /// Pascal `TAutoAdd.Solve` driver (`Solution.pas` `AUTOADDFLAG` arm): run the
    /// candidate search, write the `AutoAddLog`, instantiate the winner through
    /// the normal executive command path, re-solve, and set `GlobalResult`.
    pub(super) fn do_auto_add_solve(&mut self) {
        // Mirror the head of `TSolutionObj.Solve` (the pre-dispatch guards +
        // growth-factor set) since the AutoAdd arm is intercepted before the
        // solution dispatcher runs.
        {
            let ckt = self.circuit.as_mut().expect("gated in command()");
            ckt.is_solved = false;
            ckt.solution_was_attempted = true;
            if ckt.emerg_min_volts >= ckt.normal_min_volts {
                self.errors.push(
                    "Error: Emergency Min Voltage Must Be Less Than Normal Min Voltage! Solution Not Executed."
                        .to_string(),
                );
                return;
            }
            if ckt.solution.solution_abort {
                self.errors.push("Solution aborted.".to_string());
                return;
            }
            ckt.default_growth_factor = if ckt.solution.year == 0 {
                1.0
            } else {
                ckt.default_growth_rate.powi(ckt.solution.year - 1)
            };
        }

        // === Candidate search (needs a SolveEnv). ===
        let outcome = {
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
            auto_add_search(ckt, &mut env, output_directory)
        };
        let outcome = match outcome {
            Ok(o) => o,
            Err(e) => {
                self.errors.push(crate::diag::DssDiagnostic::msg(
                    format!("Error Encountered in Solve: {e}"),
                    Some(482),
                ));
                if let Some(ckt) = self.circuit.as_mut() {
                    ckt.solution.solution_abort = true;
                }
                return;
            }
        };

        // Write the `<CircuitName_>AutoAddLog.csv` (Pascal `GetOutputStreamEx …
        // fmCreate`). Written unconditionally, winner or not.
        self.write_auto_add_file("AutoAddLog.csv", &outcome.log_content, false);

        let is_gen = outcome.add_type == AddType::Gen;
        let min_loss_bus = outcome.min_loss_bus;

        // Winning bus name (empty string when none found).
        let bus_name = if min_loss_bus > 0 {
            let ckt = self.circuit.as_ref().expect("gated in command()");
            ckt.bus_list
                .name(min_loss_bus - 1)
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        };

        if min_loss_bus > 0 {
            // kVrat: L-L for a 3-phase device, L-N otherwise.
            let (kvrat, loss_weight, ue_weight) = {
                let ckt = self.circuit.as_ref().expect("gated in command()");
                let kv_base = ckt.buses[min_loss_bus - 1].kv_base;
                let kvrat = if outcome.min_bus_phases >= 3 {
                    kv_base * sqrt3()
                } else {
                    kv_base
                };
                (kvrat, ckt.loss_weight, ckt.ue_weight)
            };

            let command_string = if is_gen {
                let gen_name = self.unique_gen_name();
                // Pascal: `…, kW=<g>, <5.2f pf>! Factor =  <g> (<.3g>, <.3g>)`.
                format!(
                    "New generator.{gen_name}, bus1=\"{bus_name}\", phases={}, kV={}, kW={}, {}! Factor =  {} ({}, {})",
                    outcome.min_bus_phases,
                    g(kvrat, 15),
                    g(outcome.test_gen_kw, 15),
                    fixed_w(outcome.gen_pf, 5, 2),
                    g(outcome.max_loss_improve_factor, 15),
                    g(loss_weight, 3),
                    g(ue_weight, 3),
                )
            } else {
                let cap_name = self.unique_cap_name();
                format!(
                    "New Capacitor.{cap_name}, bus1=\"{bus_name}\", phases={}, kvar={}, kv={}",
                    outcome.min_bus_phases,
                    g(outcome.test_cap_kvar, 15),
                    g(kvrat, 15),
                )
            };

            // Instantiate the winner through the normal command path.
            self.command(&command_string);

            // Append the command to `<CircuitName_>AutoAdded<…>.txt`.
            let tail = if is_gen {
                "AutoAddedGenerators.txt"
            } else {
                "AutoAddedCapacitors.txt"
            };
            self.write_auto_add_file(tail, &format!("{command_string}\n"), true);

            // Force rebuilding of lists (Pascal final `SolveSnap`).
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
            if let Err(e) = solve_snap(ckt, &mut env) {
                env.errors.push(crate::diag::DssDiagnostic::msg(
                    format!("Error Encountered in Solve: {e}"),
                    Some(482),
                ));
                ckt.solution.solution_abort = true;
            }
        }

        // GlobalResult: winner bus (+ the improvement factor, GENADD only).
        self.last_result = if is_gen {
            format!("{bus_name}, {}", g(outcome.max_loss_improve_factor, 15))
        } else {
            bus_name
        };
    }

    /// `GeneratorClass.Find`-based unique name (`GetUniqueGenName`).
    fn unique_gen_name(&mut self) -> String {
        let gen_ci = self.class_by_name.get("generator").copied();
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        ckt.auto_add_obj
            .get_unique_gen_name(|name| gen_ci.is_some_and(|ci| classes[ci].set_active(name)))
    }

    /// `CapacitorClass.Find`-based unique name (`GetUniqueCapName`).
    fn unique_cap_name(&mut self) -> String {
        let cap_ci = self.class_by_name.get("capacitor").copied();
        let Dss {
            classes, circuit, ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        ckt.auto_add_obj
            .get_unique_cap_name(|name| cap_ci.is_some_and(|ci| classes[ci].set_active(name)))
    }

    /// Write (create) or append an AutoAdd side file
    /// `<OutputDirectory><CircuitName_><tail>` (Pascal `GetOutputStreamEx`
    /// `fmCreate` for the log, `AppendToFile` for the generators/capacitors
    /// echo). Not routed through `write_report` so it never touches
    /// `GlobalResult`/`@lastfile`.
    fn write_auto_add_file(&mut self, tail: &str, content: &str, append: bool) {
        let case = match self.circuit.as_ref() {
            Some(c) => c.case_name.clone(),
            None => return,
        };
        let name = format!("{case}_{tail}");
        let path = self.output_directory.join(&name);
        let res = if append {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .and_then(|mut f| f.write_all(content.as_bytes()))
        } else {
            std::fs::write(&path, content)
        };
        if let Err(e) = res {
            self.errors
                .push(format!("Error trying to write \"{}\": {e}", path.display()));
        }
    }
}
