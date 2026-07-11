//! The `Get` option command (`DoGetCmd` and its no-circuit variant). Split out
//! of `exec/set_get.rs`.

use super::*;

/// Pascal `get voltagebases` (`ExecOptions.pas`): the legal-voltage-base list
/// rendered `(b1, b2, … , )` — each value `FloatToStr`-formatted, followed by
/// `, `, closed with `)`. Shared by [`Dss::do_get_cmd`] and `Circuit.Save`'s
/// `SaveVoltageBases` (WP8.5 step 5).
pub(crate) fn voltage_bases_result(ckt: &Circuit) -> String {
    let mut result = "(".to_string();
    for v in &ckt.legal_voltage_bases {
        result.push_str(&format!("{}, ", float_to_str(*v)));
    }
    result.push(')');
    result
}

impl Dss {
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
            daisy_size,
            auto_show_export,
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
                // Pascal `ExecOptions.pas:1096` (`AppendGlobalResult(boolean)`).
                opt::LONG_LINE_CORRECTION => {
                    append_result(&mut result, yes_no(ckt.long_line_correction))
                }
                opt::ZONE_LOCK => append_result(&mut result, yes_no(ckt.zones_locked)),
                opt::UE_WEIGHT => append_result(&mut result, &float_to_str(ckt.ue_weight)),
                opt::LOSS_WEIGHT => append_result(&mut result, &float_to_str(ckt.loss_weight)),
                opt::UE_REGS => append_result(&mut result, &int_array_to_string(&ckt.ue_regs)),
                opt::LOSS_REGS => append_result(&mut result, &int_array_to_string(&ckt.loss_regs)),
                opt::VOLTAGE_BASES => {
                    // Pascal builds `(b1, b2, ... , )` replacing GlobalResult.
                    result = voltage_bases_result(ckt);
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
                opt::SEASON_RATING => append_result(&mut result, yes_no(ckt.season_rating)),
                opt::SEASON_SIGNAL => append_result(&mut result, &ckt.season_signal),
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
                // `NameIfNotNil(LoadDurCurveObj)`.
                opt::LDCURVE => append_result(
                    &mut result,
                    &ckt.load_dur_curve_obj
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
                // Pascal `ExecOptions.pas:930`: `ALL`, else each harmonic appended.
                opt::HARMONICS => {
                    if ckt.solution.do_all_harmonics {
                        append_result(&mut result, "ALL");
                    } else {
                        for h in &ckt.solution.harmonic_list {
                            append_result(&mut result, &float_to_str(*h));
                        }
                    }
                }
                opt::MAX_CONTROL_ITER => append_result(
                    &mut result,
                    &ckt.solution.max_control_iterations.to_string(),
                ),
                // Pascal `ExecOptions.pas:950-968` — the demand-interval /
                // report-switch echoes.
                opt::DEMAND_INTERVAL => {
                    append_result(&mut result, yes_no(ckt.em_di.save_demand_interval))
                }
                opt::DI_VERBOSE => append_result(&mut result, yes_no(ckt.em_di.di_verbose)),
                opt::OVERLOAD_REPORT => {
                    append_result(&mut result, yes_no(ckt.em_di.do_overload_report))
                }
                opt::VOLT_EXCEPTION_REPORT => {
                    append_result(&mut result, yes_no(ckt.em_di.do_voltage_exception_report))
                }
                opt::SAMPLE_ENERGY_METERS => {
                    append_result(&mut result, yes_no(ckt.solution.sample_the_meters))
                }
                opt::CASE_NAME => append_result(&mut result, &ckt.case_name),
                // Pascal `ExecOptions.pas:959/961` (the plot-marker echoes).
                opt::MARKER_CODE => append_result(&mut result, &ckt.node_marker_code.to_string()),
                opt::NODE_WIDTH => append_result(&mut result, &ckt.node_marker_width.to_string()),
                // The GUI plot-marker style echoes (`ExecOptions.pas:978-1041`,
                // WPG.17 Plot audit settlement); DaisySize is `%-.6g` (`:983`).
                opt::DAISY_SIZE => {
                    append_result(&mut result, &crate::report::format::g(*daisy_size, 6))
                }
                // Pascal `ExecOptions.pas:973`: `Get ShowExport` echoes the
                // stored `AutoShowExport` flag (see the Set arm).
                opt::SHOW_EXPORT => append_result(&mut result, yes_no(*auto_show_export)),
                opt::MARK_SWITCHES => append_result(&mut result, yes_no(ckt.mark_switches)),
                opt::MARK_TRANSFORMERS => append_result(&mut result, yes_no(ckt.mark_transformers)),
                opt::MARK_CAPACITORS => append_result(&mut result, yes_no(ckt.mark_capacitors)),
                opt::MARK_REGULATORS => append_result(&mut result, yes_no(ckt.mark_regulators)),
                opt::MARK_PVSYSTEMS => append_result(&mut result, yes_no(ckt.mark_pv_systems)),
                opt::MARK_STORAGE => append_result(&mut result, yes_no(ckt.mark_storage)),
                opt::MARK_FUSES => append_result(&mut result, yes_no(ckt.mark_fuses)),
                opt::MARK_RECLOSERS => append_result(&mut result, yes_no(ckt.mark_reclosers)),
                opt::MARK_RELAYS => append_result(&mut result, yes_no(ckt.mark_relays)),
                opt::SWITCH_MARKER_CODE => {
                    append_result(&mut result, &ckt.switch_marker_code.to_string())
                }
                opt::TRANS_MARKER_CODE => {
                    append_result(&mut result, &ckt.trans_marker_code.to_string())
                }
                opt::TRANS_MARKER_SIZE => {
                    append_result(&mut result, &ckt.trans_marker_size.to_string())
                }
                opt::CAP_MARKER_CODE => {
                    append_result(&mut result, &ckt.cap_marker_code.to_string())
                }
                opt::REG_MARKER_CODE => {
                    append_result(&mut result, &ckt.reg_marker_code.to_string())
                }
                opt::PV_MARKER_CODE => append_result(&mut result, &ckt.pv_marker_code.to_string()),
                opt::STORE_MARKER_CODE => {
                    append_result(&mut result, &ckt.store_marker_code.to_string())
                }
                opt::CAP_MARKER_SIZE => {
                    append_result(&mut result, &ckt.cap_marker_size.to_string())
                }
                opt::REG_MARKER_SIZE => {
                    append_result(&mut result, &ckt.reg_marker_size.to_string())
                }
                opt::PV_MARKER_SIZE => append_result(&mut result, &ckt.pv_marker_size.to_string()),
                opt::STORE_MARKER_SIZE => {
                    append_result(&mut result, &ckt.store_marker_size.to_string())
                }
                opt::FUSE_MARKER_CODE => {
                    append_result(&mut result, &ckt.fuse_marker_code.to_string())
                }
                opt::FUSE_MARKER_SIZE => {
                    append_result(&mut result, &ckt.fuse_marker_size.to_string())
                }
                opt::RECLOSER_MARKER_CODE => {
                    append_result(&mut result, &ckt.recloser_marker_code.to_string())
                }
                opt::RECLOSER_MARKER_SIZE => {
                    append_result(&mut result, &ckt.recloser_marker_size.to_string())
                }
                opt::RELAY_MARKER_CODE => {
                    append_result(&mut result, &ckt.relay_marker_code.to_string())
                }
                opt::RELAY_MARKER_SIZE => {
                    append_result(&mut result, &ckt.relay_marker_size.to_string())
                }
                opt::LOG => append_result(&mut result, yes_no(ckt.log_events)),
                opt::DEFAULT_BASE_FREQUENCY => {
                    append_result(&mut result, &(default_base_freq.round() as i64).to_string())
                }
                opt::NEGLECT_LOAD_Y => append_result(&mut result, yes_no(ckt.neglect_load_y)),
                opt::LOAD_SHAPE_CLASS => append_result(
                    &mut result,
                    &enums
                        .get(enums.load_shape_class)
                        .ordinal_to_string(ckt.active_load_shape_class),
                ),
                opt::MIN_ITERATIONS => {
                    append_result(&mut result, &ckt.solution.min_iterations.to_string())
                }
                // Pascal `ExecOptions.pas:1042-1047`: the wall-clock solve
                // timers (microseconds). Non-deterministic after a solve; `0`
                // on a fresh circuit / after `set totaltime=0`.
                opt::PROCESS_TIME => {
                    append_result(&mut result, &float_to_str(ckt.solution.solve_time_elapsed))
                }
                opt::TOTAL_TIME => {
                    append_result(&mut result, &float_to_str(ckt.solution.total_time_elapsed))
                }
                opt::STEP_TIME => {
                    append_result(&mut result, &float_to_str(ckt.solution.step_time_elapsed))
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
