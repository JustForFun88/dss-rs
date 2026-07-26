//! The `Set` option command (`DoSetCmd` and its no-circuit variant). Split out
//! of `exec/set_get.rs`.

use num_complex::Complex64;

use super::*;
use crate::elements::ckt::ElemFlags;
use crate::elements::traits::TypedStore;
use crate::solution::{ControlMode, LoadSolutionModel, RandomType};

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
    errors: &mut crate::diag::ErrorLog,
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

/// Pascal `Set InjCurrent=`/`Set ITerminal=` (ExecOptions.pas @ 0.15.0b4): parse
/// a complex vector of `NPhases` values into the active PCE's `InjCurrent` (or
/// `ITerminal`, also flagging `ITerminalUpdated`) and set `Flg.ForceInjCurrents`
/// so the injection loop / `GetCurrents` use the user-supplied values. WP-U1.9.
fn apply_force_currents(
    elem: &mut dyn CktElement,
    parser: &mut Parser,
    vars: &ParserVars,
    is_terminal: bool,
) {
    let cd = elem.cd_mut();
    let np = cd.nphases;
    let mut buf = vec![(0.0, 0.0); np];
    parser.parse_as_complex_vector(vars, &mut buf);
    let target = if is_terminal {
        &mut cd.iterminal
    } else {
        &mut cd.inj_current
    };
    for (i, &(re, im)) in buf.iter().enumerate().take(target.len()) {
        target[i] = Complex64::new(re, im);
    }
    if is_terminal {
        cd.iterminal_updated = true;
    }
    cd.flags.include(ElemFlags::FORCE_INJ_CURRENTS);
}

/// Pascal `Set YPrim=` (ExecOptions.pas @ 0.15.0b4): parse an `NConds×NConds`
/// complex matrix into the active PCE's primitive Y (column-major, matching
/// `TcMatrix.GetValuesArrayPtr`), clear `YPrimInvalid`, and set `Flg.ForceYPrim`.
/// Returns `false` on a size mismatch (Pascal error 3004) or an uninitialized
/// YPrim. WP-U1.9.
fn apply_force_yprim(elem: &mut dyn CktElement, parser: &mut Parser, vars: &ParserVars) -> bool {
    let cd = elem.cd_mut();
    let norder = cd.yorder;
    let mut buf = vec![(0.0, 0.0); norder * norder];
    if parser.parse_as_complex_matrix(vars, &mut buf, norder) != norder {
        return false;
    }
    let Some(yp) = cd.yprim.as_mut() else {
        return false;
    };
    for (v, &(re, im)) in yp.values_mut().iter_mut().zip(buf.iter()) {
        *v = Complex64::new(re, im);
    }
    cd.yprim_invalid = false;
    cd.flags.include(ElemFlags::FORCE_YPRIM);
    true
}

impl Dss {
    /// Pascal `DoSetCmd(SolveOption)`: parse `option=value` pairs, then run
    /// the solve when called from the `Solve` command.
    pub(super) fn do_set_cmd(&mut self, solve_option: i32) {
        if self.circuit.is_none() {
            self.do_set_cmd_no_circuit();
            return;
        }
        // `Set Class=`/`Set Object=` (and their `Type=`/`Element=` aliases) touch
        // whole-`self` state (`class_by_name`, `active_class`) not reachable inside
        // the field-destructure below, so they are collected in encounter order
        // here and applied right after the option loop (nothing else in a `Set`
        // command reads the active class/object mid-parse). `true` = class (activate
        // — Pascal `SetObjectClass`); `false` = object (select — `SetObject`).
        // WP-U1.1 item 5.
        let mut pending_set_active: Vec<(bool, String)> = Vec::new();
        // Pascal `ADiakoptics and (ActiveActor = 1)` fans `GETCTRLMODE` to the
        // children after a `set controlmode=`/`set maxcontroliter=` (ExecOptions.pas
        // ordinals 43/55). Tracked here, executed after the destructure block.
        let mut sync_ctrl_mode = false;
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
                auto_show_export,
                no_forms_allowed,
                no_progress_bar_form_allowed,
                current_dir,
                output_directory,
                daisy_size,
                active_ckt_element,
                class_by_name,
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

                // Pascal `Exit` semantics: the force-hook error arms abort the
                // whole `Set` command (they `DoSimpleMsg(...); Exit`), unlike a
                // normal option that logs and continues. Set by those arms.
                let mut abort = false;
                match pointer {
                    0 => {
                        // A-Diakoptics options (`Num_SubCircuits`, `Coverage`,
                        // `LinkBranches`, `UseMyLinkBranches`, `ADiakoptics`) are
                        // compiled out of the vendored/oracle build (§0.2) so they
                        // are absent from the oracle-pinned `EXEC_OPTIONS`; handled
                        // here as a recorded departure (WP-AD.2/AD.3).
                        if !crate::exec::tearing::try_set_ad_option(
                            ckt,
                            &param_name,
                            &param,
                            errors,
                        ) {
                            errors.push(format!(
                                "Unknown parameter \"{param_name}\" for Set Command"
                            ));
                        }
                    }
                    opt::HOUR => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.solution.int_hour = v;
                            // Pascal `55400a29` `Set Hour` (ExecOptions param 3)
                            // re-syncs the global seasonal-rating index, so a
                            // `solve; set hour=X; export overloads` sequence reads
                            // the new index rather than the stale solve-time one
                            // (verified on capi015: idx follows the new hour).
                            let mut store = ClassStore {
                                classes: &mut *classes,
                            };
                            crate::solution::meters::sync_seasonal_rating_idx(ckt, &mut store);
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
                                // Pascal `Round` = ties-to-even, on the hour.
                                ckt.solution.int_hour = buf[0].round_ties_even() as i32;
                                ckt.solution.t = buf[1];
                                ckt.solution.update_dbl_hour();
                            }
                            Err(e) => errors.push(e),
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
                            ckt.solution.random_type =
                                RandomType::from_ordinal(v).unwrap_or(ckt.solution.random_type);
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
                            let m = LoadSolutionModel::from_ordinal(v)
                                .unwrap_or(ckt.solution.default_load_model);
                            ckt.solution.default_load_model = m;
                            ckt.solution.load_model = m;
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
                    // Pascal `Set %mean/%stddev=` (`ExecHelper.pas:476-478`): the
                    // default daily loadshape's `Set_Mean`/`Set_StdDev` (value /
                    // 100). A silent no-op when no default daily shape exists.
                    opt::PCT_MEAN => {
                        if let Some(v) = get_dbl(parser, vars, errors)
                            && let Some(sh) = ckt.default_daily_shape_obj.as_mut()
                        {
                            sh.set_mean(v / 100.0);
                        }
                    }
                    opt::PCT_STDDEV => {
                        if let Some(v) = get_dbl(parser, vars, errors)
                            && let Some(sh) = ckt.default_daily_shape_obj.as_mut()
                        {
                            sh.set_std_dev(v / 100.0);
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
                        if let Some(at) = enum_ord(enums, enums.add_type, &param, errors)
                            .and_then(crate::circuit::AddType::from_ordinal)
                        {
                            ckt.auto_add_obj.add_type = at;
                        }
                    }
                    opt::ALLOW_DUPLICATES => ckt.duplicates_allowed = interpret_yes_no(&param),
                    opt::LONG_LINE_CORRECTION => {
                        // Pascal `ExecOptions.pas:738`.
                        ckt.long_line_correction = interpret_yes_no(&param);
                    }
                    opt::ZONE_LOCK => ckt.zones_locked = interpret_yes_no(&param),
                    // Pascal `Set genmult=` (`ExecOptions.pas:529`): the global
                    // generation multiplier (`ActiveCircuit.GenMultiplier`).
                    opt::GEN_MULT => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.gen_multiplier = v;
                        }
                    }
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
                    // Pascal `ExecOptions.pas:606`: `AutoShowExport` — the
                    // FireOffEditor auto-open after exports, a GUI no-op
                    // headless; stored for Set/Get parity only.
                    opt::SHOW_EXPORT => *auto_show_export = interpret_yes_no(&param),
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
                    // Pascal `55400a29` `Set SeasonRating`/`SeasonSignal`
                    // (ExecOptions params 114/115) each re-sync the global
                    // seasonal-rating index after mutating the toggle/signal.
                    opt::SEASON_RATING => {
                        ckt.season_rating = interpret_yes_no(&param);
                        let mut store = ClassStore {
                            classes: &mut *classes,
                        };
                        crate::solution::meters::sync_seasonal_rating_idx(ckt, &mut store);
                    }
                    opt::SEASON_SIGNAL => {
                        ckt.season_signal = param.clone();
                        let mut store = ClassStore {
                            classes: &mut *classes,
                        };
                        crate::solution::meters::sync_seasonal_rating_idx(ckt, &mut store);
                    }
                    opt::VOLTAGE_BASES => {
                        // Pascal `DoLegalVoltageBases` (1000-slot buffer).
                        let mut buf = vec![0.0; 1000];
                        match parser.parse_as_vector(vars, &mut buf, false) {
                            Ok(n) => {
                                buf.truncate(n.min(1000));
                                ckt.legal_voltage_bases = buf;
                            }
                            Err(e) => errors.push(e),
                        }
                    }
                    opt::ALGORITHM => {
                        if let Some(alg) = enum_ord(enums, enums.solve_alg, &param, errors)
                            .and_then(crate::solution::solution::SolveAlgorithm::from_ordinal)
                        {
                            ckt.solution.algorithm = alg;
                            // Pascal `ExecOptions.pas:530`: selecting NCIM forces a
                            // rebuild of its structures on the next solve.
                            if alg == crate::solution::solution::SolveAlgorithm::Ncim {
                                ckt.solution.ncim_ready = false;
                            }
                        }
                    }
                    // NCIM solver options (`ExecOptions.pas:794-797`).
                    opt::IGNORE_GEN_Q_LIMITS => {
                        ckt.solution.ncim_ignore_q_limit = interpret_yes_no(&param);
                    }
                    opt::NCIM_Q_GAIN => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            ckt.solution.ncim_gen_gain = v;
                        }
                    }
                    opt::CONTROL_MODE => {
                        if let Some(v) = enum_ord(enums, enums.control_mode, &param, errors) {
                            let m =
                                ControlMode::from_ordinal(v).unwrap_or(ckt.solution.control_mode);
                            ckt.solution.control_mode = m;
                            // always revert to last one specified in a script
                            ckt.solution.default_control_mode = m;
                            // ADiakoptics + ActiveActor=1: sync child control mode.
                            sync_ctrl_mode = true;
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
                            // Pascal `DoSimpleMsg(..., 131)` (ExecOptions.pas:484).
                            errors.push(crate::diag::DssDiagnostic::msg(
                                "Load-Duration Curve not found.",
                                Some(131),
                            ));
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
                            // ADiakoptics + ActiveActor=1: sync child iters (GETCTRLMODE).
                            sync_ctrl_mode = true;
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
                                Err(e) => errors.push(e),
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
                                    if let Some(load) = store.typed_mut::<load::Load>(lr) {
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
                    // Pascal `ExecOptions.pas:620`: `Set Daisysize=` sets the
                    // DSS-context `DaisySize` written into the plot payload.
                    opt::DAISY_SIZE => {
                        if let Some(v) = get_dbl(parser, vars, errors) {
                            *daisy_size = v;
                        }
                    }
                    // The GUI plot-marker style options (`ExecOptions.pas:615-682`):
                    // Circuit fields flowing into the plot payload's `Markers`
                    // object (WPG.17 Plot audit settlement) — headless-inert.
                    opt::MARK_SWITCHES => ckt.mark_switches = interpret_yes_no(&param),
                    opt::MARK_TRANSFORMERS => ckt.mark_transformers = interpret_yes_no(&param),
                    opt::MARK_CAPACITORS => ckt.mark_capacitors = interpret_yes_no(&param),
                    opt::MARK_REGULATORS => ckt.mark_regulators = interpret_yes_no(&param),
                    opt::MARK_PVSYSTEMS => ckt.mark_pv_systems = interpret_yes_no(&param),
                    opt::MARK_STORAGE => ckt.mark_storage = interpret_yes_no(&param),
                    opt::MARK_FUSES => ckt.mark_fuses = interpret_yes_no(&param),
                    opt::MARK_RECLOSERS => ckt.mark_reclosers = interpret_yes_no(&param),
                    opt::MARK_RELAYS => ckt.mark_relays = interpret_yes_no(&param),
                    opt::SWITCH_MARKER_CODE
                    | opt::TRANS_MARKER_CODE
                    | opt::TRANS_MARKER_SIZE
                    | opt::CAP_MARKER_CODE
                    | opt::REG_MARKER_CODE
                    | opt::PV_MARKER_CODE
                    | opt::STORE_MARKER_CODE
                    | opt::CAP_MARKER_SIZE
                    | opt::REG_MARKER_SIZE
                    | opt::PV_MARKER_SIZE
                    | opt::STORE_MARKER_SIZE
                    | opt::FUSE_MARKER_CODE
                    | opt::FUSE_MARKER_SIZE
                    | opt::RECLOSER_MARKER_CODE
                    | opt::RECLOSER_MARKER_SIZE
                    | opt::RELAY_MARKER_CODE
                    | opt::RELAY_MARKER_SIZE => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            *match pointer {
                                opt::SWITCH_MARKER_CODE => &mut ckt.switch_marker_code,
                                opt::TRANS_MARKER_CODE => &mut ckt.trans_marker_code,
                                opt::TRANS_MARKER_SIZE => &mut ckt.trans_marker_size,
                                opt::CAP_MARKER_CODE => &mut ckt.cap_marker_code,
                                opt::REG_MARKER_CODE => &mut ckt.reg_marker_code,
                                opt::PV_MARKER_CODE => &mut ckt.pv_marker_code,
                                opt::STORE_MARKER_CODE => &mut ckt.store_marker_code,
                                opt::CAP_MARKER_SIZE => &mut ckt.cap_marker_size,
                                opt::REG_MARKER_SIZE => &mut ckt.reg_marker_size,
                                opt::PV_MARKER_SIZE => &mut ckt.pv_marker_size,
                                opt::STORE_MARKER_SIZE => &mut ckt.store_marker_size,
                                opt::FUSE_MARKER_CODE => &mut ckt.fuse_marker_code,
                                opt::FUSE_MARKER_SIZE => &mut ckt.fuse_marker_size,
                                opt::RECLOSER_MARKER_CODE => &mut ckt.recloser_marker_code,
                                opt::RECLOSER_MARKER_SIZE => &mut ckt.recloser_marker_size,
                                opt::RELAY_MARKER_CODE => &mut ckt.relay_marker_code,
                                _ => &mut ckt.relay_marker_size,
                            } = v;
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
                        // Get-only. Pascal's `DoSetCmd` case has NO `108:`/`106:`
                        // arm, so it falls to `else // Ignore excess parameters`
                        // (ExecOptions.pas:759) — a pure no-op that never calls
                        // `Parser.DblValue`. Must NOT evaluate the value token: a
                        // trailing bare-quote comment (`set totaltime=0 ' timer`)
                        // parses `' timer` as a quoted string that lands on the
                        // incremented pointer 108; calling `get_dbl` would run it
                        // through the RPN interpreter → spurious "Invalid inline
                        // math entry". Leaving it unread matches OpenDSS swallowing
                        // the comment.
                    }
                    opt::NEGLECT_LOAD_Y => ckt.neglect_load_y = interpret_yes_no(&param),
                    opt::MIN_ITERATIONS => {
                        if let Some(v) = get_int(parser, vars, errors) {
                            ckt.solution.min_iterations = v;
                        }
                    }
                    // (The timing options 106/107/108 are handled above: the MMF
                    // merge carries the Pascal-faithful arms — `TotalTime` settable
                    // per ExecOptions.pas:683-684, `ProcessTime`/`StepTime` Get-only
                    // silent no-ops. The gfm branch's all-no-op arm was dropped at
                    // merge as unreachable and 107-divergent.)
                    // `Set AllowForms`/`AllowProgressBar` (ExecOptions.pas:777-780):
                    // console-form gates, inert headless — stored so the value
                    // round-trips (capi015 silently accepts; erroring diverges).
                    opt::ALLOW_FORMS => *no_forms_allowed = !interpret_yes_no(&param),
                    opt::ALLOW_PROGRESS_BAR => {
                        *no_progress_bar_form_allowed = !interpret_yes_no(&param)
                    }
                    // WP-U1.9 PCE force hooks (ExecOptions.pas @ 0.15.0b4). Their
                    // error arms `Exit` in Pascal → `abort` breaks the option loop.
                    opt::INJ_CURRENT | opt::ITERMINAL => {
                        let is_terminal = pointer == opt::ITERMINAL;
                        match active_pce(classes, ckt, *active_ckt_element) {
                            Ok(elem) => apply_force_currents(elem, parser, vars, is_terminal),
                            Err(name) => {
                                errors.push(format!("Active element ({name}) is not a PCElement."));
                                abort = true;
                            }
                        }
                    }
                    opt::YPRIM => match active_pce(classes, ckt, *active_ckt_element) {
                        Ok(elem) => {
                            if !apply_force_yprim(elem, parser, vars) {
                                errors.push(
                                    "The size of the matrix provided does not match with the \
                                     number of conductors of the active PCE."
                                        .to_string(),
                                );
                                abort = true;
                            }
                        }
                        Err(name) => {
                            errors.push(format!("Active element ({name}) is not a PCElement."));
                            abort = true;
                        }
                    },
                    opt::STATE_VAR => {
                        // `Set StateVar <element> <varname> <value>` (positional).
                        parser.next_param(vars);
                        let elem_name = parser.make_string(vars);
                        let resolved =
                            resolve_ckt_element(classes, class_by_name, parser, vars, &elem_name);
                        parser.next_param(vars);
                        let var_name = parser.make_string(vars);
                        parser.next_param(vars);
                        let value = parser.make_double(vars).unwrap_or(0.0);
                        match resolved {
                            None => {
                                errors.push(format!("Object \"{elem_name}\" not found"));
                                abort = true;
                            }
                            // Pascal checks `is TPCElement` (7103) BEFORE the
                            // NumVariables check (7101).
                            Some((ci, oi)) if !is_pce(ckt, ci, oi) => {
                                errors.push(format!(
                                    "Object \"{}.{}\" is not a valid PC element.",
                                    classes[ci].props.class_name(),
                                    classes[ci].arena[oi].data().name()
                                ));
                                abort = true;
                            }
                            Some((ci, oi)) => {
                                // Pascal `Set_Variable` on a user-model PCE reads
                                // the live `ActiveCircuit.Solution` via the model
                                // callbacks; snapshot it before the mutable elem
                                // borrow (disjoint fields: `circuit` vs `classes`).
                                let sys = crate::solution::solution::sys_ctx(ckt);
                                let elem = classes[ci]
                                    .arena
                                    .try_ckt_elem_mut(oi)
                                    .expect("resolved circuit element");
                                if elem.num_variables() == 0 {
                                    errors.push(format!(
                                        "Object \"{elem_name}\" is not a valid element for this \
                                         command. Only a selection of PC elements have state \
                                         variables."
                                    ));
                                    abort = true;
                                } else if let Some(i) = lookup_variable(elem, &var_name) {
                                    elem.set_variable(i, value, &sys);
                                } else {
                                    errors.push(format!(
                                        "State variable \"{}\" not found in \"{}.{}\".",
                                        var_name.to_ascii_lowercase(),
                                        classes[ci].props.class_name(),
                                        classes[ci].arena[oi].data().name()
                                    ));
                                    abort = true;
                                }
                            }
                        }
                    }
                    opt::ITER_NUMBER | opt::CTRL_ITER_NUMBER | opt::INTEGRATION_FLAG => {
                        // These solution counters are read-only. The Pascal
                        // source cites no `DoSimpleMsg` number here, so the
                        // diagnostic stays uncoded (`None`) — never invent one.
                        errors.push("This value is read-only.".to_string());
                        abort = true;
                    }
                    opt::PY_PATH => {
                        // pyControl co-simulation server — NOT_PORTED (§0). Loud,
                        // like capi015's "not supported in the AltDSS engine".
                        errors.push(
                            "Set PyPath= (pyControl co-simulation) is not supported in the \
                             dss-rs engine."
                                .to_string(),
                        );
                        abort = true;
                    }
                    opt::TYPE | opt::CLASS => pending_set_active.push((true, param.clone())),
                    opt::ELEMENT | opt::OBJECT => pending_set_active.push((false, param.clone())),
                    _ => {
                        let name = EXEC_OPTIONS.get(pointer - 1).copied().unwrap_or("?");
                        errors.push(format!("Set option \"{name}\" is not ported yet."));
                    }
                }

                if abort {
                    break;
                }
                param_name = parser.next_param(vars);
                param = parser.make_string(vars);
            }
        }

        // Apply the collected `Set Class`/`Set Object` ops in order (item 5).
        for (is_class, param) in pending_set_active {
            if is_class {
                self.set_object_class(&param);
            } else {
                self.set_object(&param);
            }
        }

        // `set controlmode=`/`set maxcontroliter=` while ADiakoptics is active on
        // the coordinator (ActiveActor 1) syncs the child control mode/iters.
        if sync_ctrl_mode
            && self
                .circuit
                .as_ref()
                .is_some_and(|c| c.solution.adiakoptics)
        {
            self.ad_send_get_ctrl_mode();
        }

        // `set ADiakoptics=yes` requested `ADiakopticsInit`, deferred here past
        // the option-loop borrow (it needs `&mut Dss`, not just the circuit).
        if self.circuit.as_ref().is_some_and(|c| c.ad.pending_ad_init) {
            if let Some(ckt) = self.circuit.as_mut() {
                ckt.ad.pending_ad_init = false;
            }
            self.adiakoptics_init();
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
