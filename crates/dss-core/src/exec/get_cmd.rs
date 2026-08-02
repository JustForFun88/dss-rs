//! The `Get` option command (`DoGetCmd` and its no-circuit variant). Split out
//! of `exec/set_get.rs`.

use num_complex::Complex64;

use super::*;
use crate::solution::solution::sys_ctx;
use crate::support::cmatrix::CMatrix;

/// FPC `UComplex.cstr`: a complex as a string — the real part alone when the
/// imaginary part is exactly zero, else `re±imi`. Used by the WP-U1.9
/// `Get InjCurrent`/`ITerminal`/`YPrim` renderers.
fn cstr(c: Complex64) -> String {
    if c.im == 0.0 {
        float_to_str(c.re)
    } else {
        format!(
            "{}{}{}i",
            float_to_str(c.re),
            if c.im < 0.0 { "-" } else { "+" },
            float_to_str(c.im.abs())
        )
    }
}

/// Pascal `Get InjCurrent`/`ITerminal`/`YPrim` (ExecOptions.pas @ 0.15.0b4):
/// render the active PCE's injection/terminal currents (over `NConds`) or its
/// primitive Y. Non-PCE → the EPRI-compatible "not PCE" text; an
/// unallocated buffer → "not initialized yet". WP-U1.9.
///
/// Returns `Err(text)` for the two Pascal error cases (both `AppendGlobalResult`
/// the text and then `Exit`) so the caller can reproduce the `Exit` (abort the
/// option loop); `Ok(text)` on a normal readback.
fn get_force_readback(
    classes: &mut [DssClass],
    ckt: &Circuit,
    active: Option<(usize, usize)>,
    pointer: usize,
) -> Result<String, String> {
    let elem = match active_pce(classes, ckt, active) {
        Ok(e) => e,
        Err(_) => return Err("Error, the active element is not PCE".to_string()),
    };
    let cd = elem.cd();
    let uninit = "Error, the active element is not initialized yet".to_string();
    if cd.node_ref.is_empty() {
        return Err(uninit);
    }
    if pointer == opt::YPRIM {
        return match &cd.yprim {
            Some(m) => Ok(cmatrix_to_string(m)),
            None => Err(uninit),
        };
    }
    let arr = if pointer == opt::ITERMINAL {
        &cd.iterminal
    } else {
        &cd.inj_current
    };
    if arr.len() < cd.nconds {
        return Err(uninit);
    }
    Ok(complex_array_to_string(&arr[..cd.nconds]))
}

/// Pascal `ComplexArrayToString(data, count)` (Utilities.pas @ 0.15.0b4).
fn complex_array_to_string(data: &[Complex64]) -> String {
    if data.is_empty() {
        return "[]".to_string();
    }
    let mut s = String::from("[");
    for (i, c) in data.iter().enumerate() {
        s.push_str(&cstr(*c));
        if i != data.len() - 1 {
            s.push_str(", ");
        }
    }
    s.push(']');
    s
}

/// Pascal `TcMatrix.ToString` (Ucmatrix.pas @ 0.15.0b4): `[a, b| c, d]`,
/// column-major storage read row-by-row.
fn cmatrix_to_string(m: &CMatrix) -> String {
    let n = m.order();
    if n == 0 {
        return "[]".to_string();
    }
    let mut s = String::from("[");
    for i in 0..n {
        for j in 0..n {
            s.push_str(&cstr(m.get(i, j)));
            if j != n - 1 {
                s.push_str(", ");
            }
        }
        if i != n - 1 {
            s.push_str("| ");
        }
    }
    s.push(']');
    s
}

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
            classes,
            circuit,
            option_list,
            parser,
            vars,
            enums,
            errors,
            default_base_freq,
            daisy_size,
            auto_show_export,
            no_forms_allowed,
            no_progress_bar_form_allowed,
            last_result,
            active_ckt_element,
            class_by_name,
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
            // Pascal `Exit` semantics for the force-hook error arms (see set_cmd).
            let mut abort = false;
            match pointer {
                0 => {
                    // A-Diakoptics options (§0.2 departure): intercept before the
                    // "Unknown parameter" error. `get ADiakoptics` is WP-AD.3.
                    if !crate::exec::tearing::try_get_ad_option(ckt, &param, &mut result) {
                        errors.push(format!(
                            "Unknown parameter \"{param_name}\" for Get Command"
                        ));
                    }
                }
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
                        .ordinal_to_string(ckt.solution.random_type.ordinal()),
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
                        .ordinal_to_string(ckt.solution.load_model.ordinal()),
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
                        crate::circuit::AddType::Cap => "capacitor",
                        crate::circuit::AddType::Gen => "generator",
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
                        .ordinal_to_string(ckt.solution.algorithm.ordinal()),
                ),
                // NCIM solver options (`ExecOptions.pas:1268-1271`).
                opt::IGNORE_GEN_Q_LIMITS => {
                    append_result(&mut result, yes_no(ckt.solution.ncim_ignore_q_limit))
                }
                opt::NCIM_Q_GAIN => {
                    append_result(&mut result, &float_to_str(ckt.solution.ncim_gen_gain))
                }
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
                        .ordinal_to_string(ckt.solution.control_mode.ordinal()),
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
                // Pascal `OrdinalToString(Integer(positiveSequence))`
                // (`ExecOptions.pas:919`) renders the `LongBool` -1 as `''`.
                // The authority answers the state itself
                // (`IF positiveSequence THEN AppendGlobalResult('positive')
                // ELSE AppendGlobalResult('multiphase')`, r4133
                // `Version8/Source/Executive/ExecOptions.pas:1257`), so the
                // ordinal of the state is rendered here, in both lanes.
                opt::CKT_MODEL => append_result(
                    &mut result,
                    &enums
                        .get(enums.ckt_model)
                        .ordinal_to_string(i32::from(ckt.positive_sequence)),
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
                // WP-U1.9 PCE force hooks / counters (ExecOptions.pas @ 0.15.0b4).
                opt::ITER_NUMBER => append_result(&mut result, &ckt.solution.iteration.to_string()),
                opt::CTRL_ITER_NUMBER => {
                    append_result(&mut result, &ckt.solution.control_iteration.to_string())
                }
                opt::INTEGRATION_FLAG => append_result(
                    &mut result,
                    // Pascal `Solution.DynaVars.IterationFlag` (0 = new step, 1 = same).
                    &(ckt.solution.iteration_flag as i32).to_string(),
                ),
                // `Get AllowForms`/`AllowProgressBar` (ExecOptions.pas:1246-1249,
                // `not NoFormsAllowed` / `not NoProgressBarFormAllowed`).
                opt::ALLOW_FORMS => append_result(&mut result, yes_no(!*no_forms_allowed)),
                opt::ALLOW_PROGRESS_BAR => {
                    append_result(&mut result, yes_no(!*no_progress_bar_form_allowed))
                }
                opt::INJ_CURRENT | opt::ITERMINAL | opt::YPRIM => {
                    match get_force_readback(classes, ckt, *active_ckt_element, pointer) {
                        Ok(s) => append_result(&mut result, &s),
                        Err(s) => {
                            append_result(&mut result, &s);
                            abort = true;
                        }
                    }
                }
                opt::STATE_VAR => {
                    // `Get StateVar <element> <varname>` (positional).
                    parser.next_param(vars);
                    let elem_name = parser.make_string(vars);
                    let resolved =
                        resolve_ckt_element(classes, class_by_name, parser, vars, &elem_name);
                    parser.next_param(vars);
                    let var_name = parser.make_string(vars);
                    match resolved {
                        None => {
                            errors.push(format!("Object \"{elem_name}\" not found"));
                            abort = true;
                        }
                        // Pascal checks `is TPCElement` (7103) BEFORE NumVariables.
                        Some((ci, oi)) if !is_pce(ckt, ci, oi) => {
                            errors.push(format!(
                                "Object \"{}.{}\" is not a valid PC element.",
                                classes[ci].props.class_name(),
                                classes[ci].arena[oi].data().name()
                            ));
                            abort = true;
                        }
                        Some((ci, oi)) => {
                            let nvars = classes[ci]
                                .arena
                                .try_ckt_elem(oi)
                                .map(|e| e.num_variables())
                                .unwrap_or(0);
                            let found = (1..=nvars).find(|&i| {
                                classes[ci]
                                    .arena
                                    .try_ckt_elem(oi)
                                    .expect("ckt element")
                                    .variable_name(i)
                                    .eq_ignore_ascii_case(&var_name)
                            });
                            if nvars == 0 {
                                errors.push(format!(
                                    "Object \"{elem_name}\" is not a valid element for this \
                                     command. Only a selection of PC elements have state \
                                     variables."
                                ));
                                abort = true;
                            } else if let Some(i) = found {
                                let sys = sys_ctx(ckt);
                                let node_v = ckt.solution.node_v.clone();
                                let mut states = vec![0.0; nvars];
                                classes[ci]
                                    .arena
                                    .try_ckt_elem_mut(oi)
                                    .expect("ckt element")
                                    .get_all_variables(&sys, &node_v, &mut states);
                                append_result(&mut result, &format!("{}", states[i - 1]));
                            } else {
                                errors.push(format!(
                                    "State variable \"{}\" not found in \"{}.{}\".",
                                    var_name.to_lowercase(),
                                    classes[ci].props.class_name(),
                                    classes[ci].arena[oi].data().name()
                                ));
                                abort = true;
                            }
                        }
                    }
                }
                _ => {
                    let name = EXEC_OPTIONS.get(pointer - 1).copied().unwrap_or("?");
                    errors.push(format!("Get option \"{name}\" is not ported yet."));
                }
            }
            if abort {
                break;
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
