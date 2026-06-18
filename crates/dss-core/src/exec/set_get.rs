//! The `Set`/`Get` option commands (`DoSetCmd`/`DoGetCmd` and their
//! no-circuit variants). Split out of `exec/mod.rs`.

use super::*;

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
                max_allocation_iterations,
                current_dir,
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
                            ckt.solution.year = v;
                            ckt.default_growth_factor =
                                ckt.default_growth_rate.powi(ckt.solution.year - 1);
                        }
                    }
                    opt::FREQUENCY => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.solution.set_frequency(v);
                        }
                    }
                    opt::MODE => {
                        if let Some(v) = enum_ord(enums, enums.solve_mode, &param, errors)
                            && crate::solution::set_mode(ckt, SolveMode::from_ordinal(v), errors)
                        {
                            // Pascal `Set_Mode` tail: monitor/meter resets are
                            // Phase 6 no-ops, there are no Fault elements yet
                            // (Phase 7), and `DoResetControls` runs here.
                            let mut store = ClassStore { classes };
                            let mut env = SolveEnv {
                                store: &mut store,
                                parser: aux_parser,
                                vars,
                                errors,
                            };
                            if let Err(e) =
                                crate::solution::controls::reset_all_controls(ckt, &mut env)
                            {
                                env.errors.push(format!("Error resetting controls: {e}"));
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
                    // Pascal `DoSetReduceStrategy` (ExecHelper.pas l.3049). The
                    // strategy is stored; the reduction itself is NOT_PORTED.
                    opt::REDUCE_OPTION => set_reduce_strategy(ckt, &param, errors),
                    opt::KEEP_LOAD => ckt.reduce_laterals_keep_load = interpret_yes_no(&param),
                    opt::ZMAG => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.reduction_zmag = v;
                        }
                    }
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
                            ckt.solution.set_frequency(v);
                        }
                    }
                    opt::MAX_CONTROL_ITER => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.solution.max_control_iterations = v;
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
                    opt::CASE_NAME => ckt.case_name = param.clone(),
                    opt::LOG => ckt.log_events = interpret_yes_no(&param),
                    opt::DEFAULT_BASE_FREQUENCY => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            *default_base_freq = v;
                            ckt.fundamental = v;
                            ckt.solution.set_frequency(v);
                        }
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

    /// Pascal `DoGetCmd`: append the requested option values to
    /// `GlobalResult`, comma-separated.
    pub(super) fn do_get_cmd(&mut self) {
        let Dss {
            circuit,
            option_list,
            parser,
            vars,
            enums,
            errors,
            default_base_freq,
            last_result,
            ..
        } = self;
        let ckt = circuit.as_mut().expect("gated in command()");
        let mut result = String::new();

        loop {
            let param_name = parser.next_param(vars);
            let param = parser.make_string(vars);
            if param.is_empty() {
                break;
            }
            // Params are themselves the option names to return.
            let pointer = option_list.get_command(&param).map(|i| i + 1).unwrap_or(0);
            match pointer {
                0 => errors.push(format!(
                    "Unknown parameter \"{param_name}\" for Get Command"
                )),
                opt::HOUR => append_result(&mut result, &ckt.solution.int_hour.to_string()),
                opt::SEC => append_result(&mut result, &float_to_str(ckt.solution.t)),
                opt::STEPSIZE | opt::H => append_result(&mut result, &float_to_str(ckt.solution.h)),
                opt::TIME => append_result(
                    &mut result,
                    &format!(
                        "[ {}, {} ] !... {} (hours)",
                        ckt.solution.int_hour,
                        float_to_str(ckt.solution.t),
                        float_to_str(ckt.solution.dbl_hour)
                    ),
                ),
                opt::YEAR => append_result(&mut result, &ckt.solution.year.to_string()),
                opt::FREQUENCY => append_result(&mut result, &float_to_str(ckt.solution.frequency)),
                opt::MODE => append_result(
                    &mut result,
                    &enums
                        .get(enums.solve_mode)
                        .ordinal_to_string(ckt.solution.mode.ordinal()),
                ),
                opt::RANDOM => append_result(
                    &mut result,
                    &enums
                        .get(enums.random_mode)
                        .ordinal_to_string(ckt.solution.random_type),
                ),
                opt::NUMBER => {
                    append_result(&mut result, &ckt.solution.number_of_times.to_string())
                }
                opt::TOLERANCE => append_result(
                    &mut result,
                    &float_to_str(ckt.solution.convergence_tolerance),
                ),
                opt::MAXITERATIONS => {
                    append_result(&mut result, &ckt.solution.max_iterations.to_string())
                }
                opt::LOADMODEL => append_result(
                    &mut result,
                    &enums
                        .get(enums.default_load_model)
                        .ordinal_to_string(ckt.solution.load_model),
                ),
                opt::LOADMULT => append_result(&mut result, &float_to_str(ckt.load_multiplier)),
                opt::NORMVMINPU => append_result(&mut result, &float_to_str(ckt.normal_min_volts)),
                opt::NORMVMAXPU => append_result(&mut result, &float_to_str(ckt.normal_max_volts)),
                opt::EMERGVMINPU => append_result(&mut result, &float_to_str(ckt.emerg_min_volts)),
                opt::EMERGVMAXPU => append_result(&mut result, &float_to_str(ckt.emerg_max_volts)),
                opt::PCT_GROWTH => append_result(
                    &mut result,
                    &float_to_str((ckt.default_growth_rate - 1.0) * 100.0),
                ),
                opt::GEN_KW => append_result(&mut result, &float_to_str(ckt.auto_add_obj.gen_kw)),
                opt::GEN_PF => append_result(&mut result, &float_to_str(ckt.auto_add_obj.gen_pf)),
                opt::CAP_KVAR => {
                    append_result(&mut result, &float_to_str(ckt.auto_add_obj.cap_kvar))
                }
                opt::ADD_TYPE => append_result(
                    &mut result,
                    // Pascal echoes the lowercase device word, not the enum name.
                    match ckt.auto_add_obj.add_type {
                        crate::circuit::CAPADD => "capacitor",
                        _ => "generator",
                    },
                ),
                opt::ALLOW_DUPLICATES => append_result(&mut result, yes_no(ckt.duplicates_allowed)),
                opt::ZONE_LOCK => append_result(&mut result, yes_no(ckt.zones_locked)),
                opt::UE_WEIGHT => append_result(&mut result, &float_to_str(ckt.ue_weight)),
                opt::LOSS_WEIGHT => append_result(&mut result, &float_to_str(ckt.loss_weight)),
                opt::UE_REGS => append_result(&mut result, &int_array_to_string(&ckt.ue_regs)),
                opt::LOSS_REGS => append_result(&mut result, &int_array_to_string(&ckt.loss_regs)),
                opt::VOLTAGE_BASES => {
                    // Pascal builds `(b1, b2, ... , )` replacing GlobalResult.
                    result = "(".to_string();
                    for v in &ckt.legal_voltage_bases {
                        result.push_str(&format!("{}, ", float_to_str(*v)));
                    }
                    result.push(')');
                }
                opt::ALGORITHM => append_result(
                    &mut result,
                    &enums
                        .get(enums.solve_alg)
                        .ordinal_to_string(ckt.solution.algorithm),
                ),
                opt::AUTO_BUS_LIST => {
                    for name in &ckt.auto_add_bus_list {
                        append_result(&mut result, name);
                    }
                }
                opt::REDUCE_OPTION => append_result(&mut result, &ckt.reduction_strategy_string),
                opt::KEEP_LOAD => append_result(&mut result, yes_no(ckt.reduce_laterals_keep_load)),
                opt::ZMAG => append_result(&mut result, &float_to_str(ckt.reduction_zmag)),
                opt::CONTROL_MODE => append_result(
                    &mut result,
                    &enums
                        .get(enums.control_mode)
                        .ordinal_to_string(ckt.solution.control_mode),
                ),
                opt::DEFAULT_DAILY => append_result(
                    &mut result,
                    &ckt.default_daily_shape_obj
                        .as_ref()
                        .map(|s| s.data().name().to_string())
                        .unwrap_or_default(),
                ),
                opt::DEFAULT_YEARLY => append_result(
                    &mut result,
                    &ckt.default_yearly_shape_obj
                        .as_ref()
                        .map(|s| s.data().name().to_string())
                        .unwrap_or_default(),
                ),
                opt::CKT_MODEL => append_result(
                    &mut result,
                    &enums
                        .get(enums.ckt_model)
                        .ordinal_to_string(ckt.positive_sequence as i32),
                ),
                opt::PRICE_SIGNAL => append_result(&mut result, &float_to_str(ckt.price_signal)),
                opt::PRICE_CURVE => append_result(
                    &mut result,
                    &ckt.price_curve_obj
                        .as_ref()
                        .map(|s| s.data().name().to_string())
                        .unwrap_or_default(),
                ),
                opt::BASE_FREQUENCY => append_result(&mut result, &float_to_str(ckt.fundamental)),
                opt::MAX_CONTROL_ITER => append_result(
                    &mut result,
                    &ckt.solution.max_control_iterations.to_string(),
                ),
                opt::CASE_NAME => append_result(&mut result, &ckt.case_name),
                opt::LOG => append_result(&mut result, yes_no(ckt.log_events)),
                opt::DEFAULT_BASE_FREQUENCY => {
                    append_result(&mut result, &(default_base_freq.round() as i64).to_string())
                }
                opt::NEGLECT_LOAD_Y => append_result(&mut result, yes_no(ckt.neglect_load_y)),
                opt::MIN_ITERATIONS => {
                    append_result(&mut result, &ckt.solution.min_iterations.to_string())
                }
                _ => {
                    let name = EXEC_OPTIONS.get(pointer - 1).copied().unwrap_or("?");
                    errors.push(format!("Get option \"{name}\" is not ported yet."));
                }
            }
        }
        *last_result = result;
    }

    /// Pascal `DoGetCmd_NoCircuit`.
    pub(super) fn do_get_cmd_no_circuit(&mut self) {
        let Dss {
            option_list,
            parser,
            vars,
            errors,
            default_base_freq,
            last_result,
            ..
        } = self;
        let mut result = String::new();
        loop {
            parser.next_param(vars);
            let param = parser.make_string(vars);
            if param.is_empty() {
                break;
            }
            let pointer = option_list.get_command(&param).map(|i| i + 1).unwrap_or(0);
            match pointer {
                opt::DEFAULT_BASE_FREQUENCY => {
                    append_result(&mut result, &(default_base_freq.round() as i64).to_string())
                }
                _ => {
                    errors.push(
                        "You must create a new circuit object first: \"new circuit.mycktname\" to execute this Get command."
                            .to_string(),
                    );
                    return;
                }
            }
        }
        *last_result = result;
    }
}
