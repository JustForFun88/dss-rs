//! The `Set` option command (`DoSetCmd` and its no-circuit variant). Split out
//! of `exec/set_get.rs`.

use super::*;

/// Pascal `SetDataPath` (DSSGlobals.pas:540): create the dir if missing (#907 on
/// failure → leave dirs unchanged), then point both the working dir and the
/// report `OutputDirectory` at it. Allowed with or without a circuit (it touches
/// the DSS context, not the circuit). The non-writable-dir → scratch fallback is
/// NOT_PORTED — an environment-dependent I/O rescue (`DSSGlobals.pas:561-568`
/// redirects `OutputDirectory` to the per-user `GetDefaultScratchDirectory`
/// appdata dir when the target isn't writable), machine-state-dependent and not
/// oracle-pinnable; the port keeps `OutputDirectory` on the requested dir, so a
/// later write fails loudly instead of landing in a hidden scratch dir. Empty
/// `DataPath=` is a no-op here (Pascal clears DataDirectory; unexercised).
///
/// Uses single-level `create_dir` (not `create_dir_all`) to match Pascal's RTL
/// `CreateDir`, which fails — #907, dirs unchanged — when a *parent* is missing.
///
/// A relative path resolves against the engine's `current_dir`: Pascal's
/// `DirectoryExists`/`CreateDir` resolve against the *process* cwd, which
/// tracks `CurrentDSSDir` (`SetCurrentDSSDir` really chdirs —
/// `DSS_CAPI_ALLOW_CHANGE_DIR` defaults on), and a `Compile` moves it to the
/// deck's directory; Rust models that cwd virtually in `current_dir`, so the
/// join is the faithful equivalent.
fn apply_data_path(
    param: &str,
    current_dir: &mut PathBuf,
    output_directory: &mut PathBuf,
    errors: &mut Vec<String>,
) {
    if param.is_empty() {
        return;
    }
    let p = current_dir.join(param); // an absolute `param` wins the join verbatim
    if p.is_dir() || std::fs::create_dir(&p).is_ok() {
        *current_dir = p.clone();
        *output_directory = p;
    } else {
        errors.push(format!("Cannot create directory: \"{param}\""));
    }
}

impl Dss {
    /// Pascal `DoSetCmd(SolveOption)`: parse `option=value` pairs, then run
    /// the solve when called from the `Solve` command.
    pub(super) fn do_set_cmd(&mut self, solve_option: i32) {
        if self.circuit.is_none() {
            self.do_set_cmd_no_circuit();
            return;
        }
        {
            let Dss {
                classes,
                circuit,
                option_list,
                parser,
                aux_parser,
                vars,
                enums,
                errors,
                default_base_freq,
                default_earth_model,
                max_allocation_iterations,
                current_dir,
                output_directory,
                ..
            } = self;
            let ckt = circuit.as_mut().expect("checked above");

            let mut pointer: usize = 0;
            let mut param_name = parser.next_param(vars);
            let mut param = parser.make_string(vars);
            while !param.is_empty() {
                if param_name.is_empty() {
                    pointer += 1;
                } else {
                    pointer = option_list
                        .get_command(&param_name)
                        .map(|i| i + 1)
                        .unwrap_or(0);
                }

                match pointer {
                    0 => errors.push(format!(
                        "Unknown parameter \"{param_name}\" for Set Command"
                    )),
                    opt::HOUR => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.solution.int_hour = v;
                        }
                    }
                    opt::SEC => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.solution.t = v;
                        }
                    }
                    opt::STEPSIZE | opt::H => {
                        ckt.solution.h = interpret_time_step_size(&param, ckt.solution.h, errors);
                        ckt.solution.interval_hrs = ckt.solution.h / 3600.0;
                    }
                    opt::TIME => {
                        // Pascal `Set_Time`: parse `[hour, sec]` as a 2-vector.
                        let mut buf = vec![0.0; 2];
                        match parser.parse_as_vector(vars, &mut buf, false) {
                            Ok(_) => {
                                // TODO(compat): FPC banker's `Round` on the hour.
                                ckt.solution.int_hour = buf[0].round_ties_even() as i32;
                                ckt.solution.t = buf[1];
                                ckt.solution.update_dbl_hour();
                            }
                            Err(e) => errors.push(e.message().to_string()),
                        }
                    }
                    opt::YEAR => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            // Pascal `TSolutionObj.Set_Year` (Solution.pas:2266):
                            // close any open demand-interval files, restart the
                            // clock, then `EnergyMeterClass.ResetAll` (which
                            // rebuilds the DI_yr_<year> directory).
                            let mut store = ClassStore { classes };
                            if ckt.em_di.di_files_are_open {
                                crate::solution::meters::close_all_di_files(
                                    ckt, &mut store, errors,
                                );
                            }
                            ckt.solution.year = v;
                            ckt.solution.int_hour = 0;
                            ckt.solution.t = 0.0;
                            ckt.solution.update_dbl_hour();
                            crate::solution::meters::reset_all_meters(
                                ckt,
                                &mut store,
                                output_directory,
                                errors,
                            );
                            ckt.default_growth_factor =
                                ckt.default_growth_rate.powi(ckt.solution.year - 1);
                        }
                    }
                    opt::FREQUENCY => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.solution.set_frequency(v, ckt.fundamental);
                        }
                    }
                    opt::MODE => {
                        if let Some(v) = enum_ord(enums, enums.solve_mode, &param, errors) {
                            let new_mode = SolveMode::from_ordinal(v);
                            let was_dynamic = ckt.solution.is_dynamic_model;
                            // Pascal `OK_for_Dynamics` (Solution.pas l.2188) seeds the
                            // machine states with `calcInitialMachineStates` *before*
                            // `Set_Mode` commits the new mode / `IsDynamicModel` / `h`
                            // — so each machine's `InitStateVars` captures its operating
                            // point from the **power-flow** state (`ComputeIterminal`
                            // must see the pre-dynamics current branch, not the dynamic
                            // Norton one). Reproduce that timing here, while
                            // `is_dynamic_model` is still the old value, on a fresh entry
                            // into a dynamics mode (Dynamic/MonteFault/FaultStudy) from a
                            // solved circuit — the same `not IsDynamicModel and
                            // ValueIsDynamic and IsSolved` condition `set_mode` re-checks
                            // before committing. (MonteFault/FaultStudy *solves* are
                            // WP7.9, but the state init is harmless + faithful for them.)
                            if !was_dynamic
                                && matches!(
                                    new_mode,
                                    SolveMode::Dynamic
                                        | SolveMode::MonteFault
                                        | SolveMode::FaultStudy
                                )
                                && ckt.is_solved
                            {
                                let mut store = ClassStore {
                                    classes: &mut *classes,
                                };
                                let mut env = SolveEnv {
                                    store: &mut store,
                                    parser: &mut *aux_parser,
                                    vars,
                                    errors: &mut *errors,
                                };
                                crate::solution::calc_initial_machine_states(ckt, &mut env);
                            }
                            if crate::solution::set_mode(ckt, new_mode, errors) {
                                let mut store = ClassStore { classes };
                                let mut env = SolveEnv {
                                    store: &mut store,
                                    parser: aux_parser,
                                    vars,
                                    errors,
                                };
                                // Pascal `OK_for_Harmonics`: entering harmonics
                                // mode initialises each PC element's harmonic base
                                // values from the present fundamental solution
                                // (`set_mode` already enforced solved@fundamental).
                                // The return is discarded: Pascal `OK_for_Harmonics`
                                // returns false (→ `Set_Mode` `Exit`, no mode change,
                                // no reset tail) only if an element aborts, which no
                                // ported `init_harmonics` does (see harmonics.rs);
                                // honour it when the DER abort path is ported.
                                if matches!(new_mode, SolveMode::Harmonic | SolveMode::HarmonicT) {
                                    crate::solution::initialize_for_harmonics(ckt, &mut env);
                                }
                                // Pascal `Set_Mode` tail (Solution.pas l.2132):
                                // reset monitors, meters, faults, controls — in
                                // that order. The monitor reset rebuilds each
                                // header from the now-committed `IsHarmonicModel`,
                                // so entering harmonics relabels the two time
                                // columns `Freq`/`Harmonic`; it also clears any
                                // samples accumulated under the previous mode.
                                crate::solution::monitors::reset_all_monitors(ckt, &mut env);
                                crate::solution::meters::reset_all_meters(
                                    ckt,
                                    env.store,
                                    output_directory,
                                    env.errors,
                                );
                                crate::solution::faults::reset_faults(ckt, &mut env);
                                if let Err(e) =
                                    crate::solution::controls::reset_all_controls(ckt, &mut env)
                                {
                                    env.errors.push(format!("Error resetting controls: {e}"));
                                }
                            }
                        }
                    }
                    opt::RANDOM => {
                        if let Some(v) = enum_ord(enums, enums.random_mode, &param, errors) {
                            ckt.solution.random_type = v;
                        }
                    }
                    opt::NUMBER => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.solution.number_of_times = v;
                        }
                    }
                    opt::TOLERANCE => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.solution.convergence_tolerance = v;
                        }
                    }
                    opt::MAXITERATIONS => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.solution.max_iterations = v;
                        }
                    }
                    opt::LOADMODEL => {
                        if let Some(v) = enum_ord(enums, enums.default_load_model, &param, errors) {
                            ckt.solution.default_load_model = v;
                            ckt.solution.load_model = v;
                        }
                    }
                    opt::LOADMULT => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.load_multiplier = v;
                            ckt.solution.system_y_changed = true;
                        }
                    }
                    opt::NORMVMINPU => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.normal_min_volts = v;
                        }
                    }
                    opt::NORMVMAXPU => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.normal_max_volts = v;
                        }
                    }
                    opt::EMERGVMINPU => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.emerg_min_volts = v;
                        }
                    }
                    opt::EMERGVMAXPU => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.emerg_max_volts = v;
                        }
                    }
                    opt::PCT_GROWTH => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.default_growth_rate = 1.0 + v / 100.0;
                            ckt.default_growth_factor =
                                ckt.default_growth_rate.powi(ckt.solution.year - 1);
                        }
                    }
                    // Pascal `Set GenkW/GenPF/Capkvar/AddType=`: the auto-add
                    // option object (`Circuit.AutoAddObj`). The auto-add solve
                    // itself is NOT_PORTED (see circuit/auto_add.rs).
                    opt::GEN_KW => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.auto_add_obj.gen_kw = v;
                        }
                    }
                    opt::GEN_PF => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.auto_add_obj.gen_pf = v;
                        }
                    }
                    opt::CAP_KVAR => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.auto_add_obj.cap_kvar = v;
                        }
                    }
                    opt::ADD_TYPE => {
                        if let Some(v) = enum_ord(enums, enums.add_type, &param, errors) {
                            ckt.auto_add_obj.add_type = v;
                        }
                    }
                    opt::ALLOW_DUPLICATES => ckt.duplicates_allowed = interpret_yes_no(&param),
                    opt::ZONE_LOCK => ckt.zones_locked = interpret_yes_no(&param),
                    opt::UE_WEIGHT => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.ue_weight = v;
                        }
                    }
                    opt::LOSS_WEIGHT => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.loss_weight = v;
                        }
                    }
                    // Pascal `parseIntArray` (ExecOptions.pas l.350) via AuxParser.
                    opt::UE_REGS => ckt.ue_regs = parse_int_array(aux_parser, vars, &param, errors),
                    opt::LOSS_REGS => {
                        ckt.loss_regs = parse_int_array(aux_parser, vars, &param, errors)
                    }
                    // Pascal `Set Trapezoidal=`: the meter integration rule
                    // (reset to false by `Set mode=`).
                    opt::TRAPEZOIDAL => ckt.trapezoidal_integration = interpret_yes_no(&param),
                    // Pascal `DoAutoAddBusList` (ExecHelper.pas l.1986).
                    opt::AUTO_BUS_LIST => do_auto_add_bus_list(
                        aux_parser,
                        vars,
                        current_dir,
                        &param,
                        &mut ckt.auto_add_bus_list,
                        errors,
                    ),
                    // Pascal `DoKeeperBusList` (ExecHelper.pas l.2035): mark
                    // KeepList buses (cumulative) so reduction won't eliminate them.
                    opt::KEEP_LIST => {
                        do_keeper_bus_list(aux_parser, vars, current_dir, &param, ckt, errors)
                    }
                    // Pascal `DoSetReduceStrategy` (ExecHelper.pas l.3049).
                    opt::REDUCE_OPTION => set_reduce_strategy(ckt, &param, errors),
                    opt::KEEP_LOAD => ckt.reduce_laterals_keep_load = interpret_yes_no(&param),
                    opt::ZMAG => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.reduction_zmag = v;
                        }
                    }
                    // Pascal `ExecOptions.pas:696/698` (GAPS_PLAN WPG.11): the
                    // option is spelled `SeasonRating`, the global it sets is
                    // `SeasonalRating` (probe-proven: `Set SeasonalRating` is
                    // error #130, unknown parameter).
                    opt::SEASON_RATING => ckt.season_rating = interpret_yes_no(&param),
                    opt::SEASON_SIGNAL => ckt.season_signal = param.clone(),
                    opt::VOLTAGE_BASES => {
                        // Pascal `DoLegalVoltageBases` (1000-slot buffer).
                        let mut buf = vec![0.0; 1000];
                        match parser.parse_as_vector(vars, &mut buf, false) {
                            Ok(n) => {
                                buf.truncate(n.min(1000));
                                ckt.legal_voltage_bases = buf;
                            }
                            Err(e) => errors.push(e.message().to_string()),
                        }
                    }
                    opt::ALGORITHM => {
                        if let Some(v) = enum_ord(enums, enums.solve_alg, &param, errors) {
                            ckt.solution.algorithm = v;
                        }
                    }
                    opt::CONTROL_MODE => {
                        if let Some(v) = enum_ord(enums, enums.control_mode, &param, errors) {
                            ckt.solution.control_mode = v;
                            // always revert to last one specified in a script
                            ckt.solution.default_control_mode = v;
                        }
                    }
                    opt::DEFAULT_DAILY => {
                        // Pascal: only replace when the shape is found.
                        if let Some(shape) = find_load_shape(classes, &param) {
                            ckt.default_daily_shape_obj = Some(shape);
                        }
                    }
                    opt::DEFAULT_YEARLY => {
                        if let Some(shape) = find_load_shape(classes, &param) {
                            ckt.default_yearly_shape_obj = Some(shape);
                        }
                    }
                    opt::LDCURVE => {
                        // Pascal assigns `LoadShapeClass.Find`'s result (NIL
                        // on miss) first (`ExecOptions.pas` ordinal 27).
                        ckt.load_dur_curve_obj = find_load_shape(classes, &param);
                        if ckt.load_dur_curve_obj.is_none() {
                            errors.push("Load-Duration Curve not found.".to_string());
                        }
                    }
                    opt::CKT_MODEL => {
                        if let Some(v) = enum_ord(enums, enums.ckt_model, &param, errors) {
                            ckt.positive_sequence = v != 0;
                        }
                    }
                    opt::PRICE_SIGNAL => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.price_signal = v;
                        }
                    }
                    opt::PRICE_CURVE => {
                        // Pascal assigns Find()'s result (NIL on miss) first.
                        ckt.price_curve_obj = find_price_shape(classes, &param);
                        if ckt.price_curve_obj.is_none() {
                            errors.push(format!("Priceshape.{param} not found."));
                        }
                    }
                    opt::BASE_FREQUENCY => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.fundamental = v; // Set Base Frequency for system
                            ckt.solution.set_frequency(v, ckt.fundamental);
                        }
                    }
                    opt::MAX_CONTROL_ITER => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.solution.max_control_iterations = v;
                        }
                    }
                    // Pascal `DoHarmonicsList` (ExecHelper.pas l.2687): `ALL`
                    // sweeps every spectrum frequency; otherwise the value is a
                    // vector of harmonics (zero-filled) used in place of `ALL`.
                    opt::HARMONICS => {
                        if param.eq_ignore_ascii_case("ALL") {
                            ckt.solution.do_all_harmonics = true;
                        } else {
                            ckt.solution.do_all_harmonics = false;
                            let mut buf = vec![0.0; 100];
                            match parser.parse_as_vector(vars, &mut buf, false) {
                                Ok(n) => {
                                    buf.truncate(n.min(100));
                                    ckt.solution.harmonic_list = buf;
                                }
                                Err(e) => errors.push(e.message().to_string()),
                            }
                        }
                    }
                    // Pascal `DoSetAllocationFactors` (ExecHelper.pas l.2651):
                    // set every load's kVA allocation factor (ConnectedkVA spec).
                    opt::ALLOCATION_FACTORS => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            if v <= 0.0 {
                                errors.push(
                                    "Allocation Factor must be greater than zero.".to_string(),
                                );
                            } else {
                                let mut store = ClassStore {
                                    classes: &mut classes[..],
                                };
                                for &lr in &ckt.loads {
                                    if let Some(load) =
                                        store.obj_mut(lr).as_any_mut().downcast_mut::<load::Load>()
                                    {
                                        load.set_kva_allocation_factor(v);
                                    }
                                }
                            }
                        }
                    }
                    opt::NUM_ALLOC_ITERATIONS => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            *max_allocation_iterations = v;
                        }
                    }
                    // Pascal `Set DemandInterval=` / `DIVerbose=`
                    // (`ExecOptions.pas:581/588`): both property setters run
                    // `EnergyMeterClass.ResetAll` (closing + re-creating the DI
                    // machinery under the new switch).
                    opt::DEMAND_INTERVAL | opt::DI_VERBOSE => {
                        let value = interpret_yes_no(&param);
                        if pointer == opt::DEMAND_INTERVAL {
                            ckt.em_di.save_demand_interval = value;
                        } else {
                            ckt.em_di.di_verbose = value;
                        }
                        let mut store = ClassStore { classes };
                        crate::solution::meters::reset_all_meters(
                            ckt,
                            &mut store,
                            output_directory,
                            errors,
                        );
                    }
                    opt::OVERLOAD_REPORT => ckt.em_di.do_overload_report = interpret_yes_no(&param),
                    opt::VOLT_EXCEPTION_REPORT => {
                        ckt.em_di.do_voltage_exception_report = interpret_yes_no(&param)
                    }
                    // Pascal `ExecOptions.pas:686` — force/suppress the meter
                    // sampling in the time-series solve loops.
                    opt::SAMPLE_ENERGY_METERS => {
                        ckt.solution.sample_the_meters = interpret_yes_no(&param)
                    }
                    opt::CASE_NAME => ckt.case_name = param.clone(),
                    // GUI plot-marker style state (Circuit.pas fields; headless-
                    // inert except the `Export Profile` NodeCode/NodeWidth echo).
                    opt::MARKER_CODE => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.node_marker_code = v;
                        }
                    }
                    opt::NODE_WIDTH => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.node_marker_width = v;
                        }
                    }
                    opt::DATA_PATH => {
                        apply_data_path(&param, current_dir, output_directory, errors)
                    }
                    opt::LOG => ckt.log_events = interpret_yes_no(&param),
                    opt::DEFAULT_BASE_FREQUENCY => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            *default_base_freq = v;
                            ckt.fundamental = v;
                            ckt.solution.set_frequency(v, ckt.fundamental);
                        }
                    }
                    opt::LOAD_SHAPE_CLASS => {
                        // Pascal `ExecOptions.pas:628`: `Set LoadShapeClass=` sets
                        // `Circuit.ActiveLoadShapeClass` — the one shape class the
                        // GENERALTIME/DYNAMICMODE nominal dispatch consults.
                        match enums.get(enums.load_shape_class).string_to_ordinal(&param) {
                            Ok(v) => ckt.active_load_shape_class = v,
                            Err(e) => errors.push(e.to_string()),
                        }
                    }
                    opt::EARTH_MODEL => {
                        // Pascal `ExecOptions.pas:630`: `Set EarthModel=` sets the
                        // context default copied into each new `TLineObj.FEarthModel`.
                        match enums.get(enums.earth_model).string_to_ordinal(&param) {
                            Ok(v) => *default_earth_model = v,
                            Err(e) => errors.push(e.to_string()),
                        }
                    }
                    // Pascal `ExecOptions.pas:683-684`: only `TotalTime` is
                    // settable. `ProcessTime`/`StepTime` are Get-only — the
                    // Pascal case falls to `else // Ignore excess parameters`
                    // (silent no-op), so they must NOT hit the "not ported" arm.
                    opt::TOTAL_TIME => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.solution.total_time_elapsed = v;
                        }
                    }
                    opt::PROCESS_TIME | opt::STEP_TIME => {
                        // Get-only: consume the value, no effect (Pascal no-op).
                        let _ = get_dbl(parser, vars, errors);
                    }
                    opt::NEGLECT_LOAD_Y => ckt.neglect_load_y = interpret_yes_no(&param),
                    opt::MIN_ITERATIONS => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.solution.min_iterations = v;
                        }
                    }
                    _ => {
                        let name = EXEC_OPTIONS.get(pointer - 1).copied().unwrap_or("?");
                        errors.push(format!("Set option \"{name}\" is not ported yet."));
                    }
                }

                param_name = parser.next_param(vars);
                param = parser.make_string(vars);
            }
        }

        if solve_option == 1 {
            self.do_solve_cmd();
        }
    }

    /// Pascal `DoSetCmd_NoCircuit`: the few global options legal without a
    /// circuit; anything else is the error-301 message.
    pub(super) fn do_set_cmd_no_circuit(&mut self) {
        let Dss {
            option_list,
            parser,
            vars,
            errors,
            default_base_freq,
            current_dir,
            output_directory,
            ..
        } = self;
        let mut pointer: usize = 0;
        let mut param_name = parser.next_param(vars);
        let mut param = parser.make_string(vars);
        while !param.is_empty() {
            if param_name.is_empty() {
                pointer += 1;
            } else {
                pointer = option_list
                    .get_command(&param_name)
                    .map(|i| i + 1)
                    .unwrap_or(0);
            }
            match pointer {
                0 => errors.push(format!(
                    "Unknown parameter \"{param_name}\" for Set Command"
                )),
                opt::DEFAULT_BASE_FREQUENCY => {
                    if let Some(v) = get_dbl(parser, vars, errors) {
                        *default_base_freq = v;
                    }
                }
                // `Set DataPath=` is legal before a circuit exists (Pascal
                // operates on the DSS context, not the circuit) — a common
                // pattern at the top of a script.
                opt::DATA_PATH => apply_data_path(&param, current_dir, output_directory, errors),
                _ => {
                    errors.push(
                        "You must create a new circuit object first: \"new circuit.mycktname\" to execute this Set command."
                            .to_string(),
                    );
                    return;
                }
            }
            param_name = parser.next_param(vars);
            param = parser.make_string(vars);
        }
    }
}
