//! Port of `Common/Solution.pas` (`TSolutionObj`), Phase 3 subset: the
//! snapshot power-flow path (`Solve` → `SolveSnap` → `SolveCircuit` →
//! `DoPFLOWsolution` → `DoNormalSolution`), `Converged`, the injection
//! machinery, `SolveSystem`, `SolveZeroLoadSnapShot`, `SetVoltageBases`,
//! and `SolveDirect`.
//!
//! Pascal reaches elements through `ActiveCircuit` pointer lists; here the
//! free functions take the [`Circuit`] plus a [`SolveEnv`] carrying the
//! element store, the parser (for bus reprocessing) and the error sink.

use num_complex::Complex64;

use dss_parser::{Parser, ParserVars};
use dss_sparse::SparseSet;

use crate::circuit::Circuit;
use crate::elements::traits::{ElemStore, InjCtx, SysCtx};
use crate::solution::control_queue::ControlQueue;
use crate::solution::event_log::EventLog;
use crate::solution::ymatrix::{BuildOption, build_y_matrix, initialize_node_vbase};
use crate::util::sqrt3;

/// Pascal `TSolveMode` (Phase 3 implements Snapshot and Direct).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SolveMode {
    #[default]
    Snapshot = 0,
    Daily = 1,
    Yearly = 2,
    Monte1 = 3,
    LD1 = 4,
    PeakDay = 5,
    DutyCycle = 6,
    Direct = 7,
    MonteFault = 8,
    FaultStudy = 9,
    Monte2 = 10,
    Monte3 = 11,
    LD2 = 12,
    AutoAdd = 13,
    Dynamic = 14,
    Harmonic = 15,
    Time = 16,
    HarmonicT = 17,
}

impl SolveMode {
    /// Ordinal as dss-python reports `Solution.Mode` (the COM ordering above).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// Pascal `TSolveMode(ordinal)` cast (the `Set mode=` path). Out-of-range
    /// ordinals cannot come from the SolveModeEnum, so fall back to Snapshot.
    pub fn from_ordinal(value: i32) -> Self {
        match value {
            1 => Self::Daily,
            2 => Self::Yearly,
            3 => Self::Monte1,
            4 => Self::LD1,
            5 => Self::PeakDay,
            6 => Self::DutyCycle,
            7 => Self::Direct,
            8 => Self::MonteFault,
            9 => Self::FaultStudy,
            10 => Self::Monte2,
            11 => Self::Monte3,
            12 => Self::LD2,
            13 => Self::AutoAdd,
            14 => Self::Dynamic,
            15 => Self::Harmonic,
            16 => Self::Time,
            17 => Self::HarmonicT,
            _ => Self::Snapshot,
        }
    }
}

/// Load model codes (DSSGlobals.pas).
pub const POWERFLOW: i32 = 1;
pub const ADMITTANCE: i32 = 2;

/// Algorithm codes.
pub const NORMALSOLVE: i32 = 0;
pub const NEWTONSOLVE: i32 = 1;

/// Control modes (DSSGlobals.pas).
pub const CONTROLSOFF: i32 = -1;
pub const CTRLSTATIC: i32 = 0;
pub const EVENTDRIVEN: i32 = 1;
pub const TIMEDRIVEN: i32 = 2;
pub const MULTIRATE: i32 = 3;

/// Which sparse set is active (`hY = hYsystem | hYseries`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveY {
    System,
    Series,
}

/// The solve environment: everything outside the circuit that the solution
/// flow needs (PORTING_PLAN.md §2.1 disjoint-borrow context).
pub struct SolveEnv<'a> {
    pub store: &'a mut dyn ElemStore,
    pub parser: &'a mut Parser,
    pub vars: &'a ParserVars,
    /// `DoSimpleMsg` sink.
    pub errors: &'a mut Vec<String>,
}

/// `ESolveError`-style hard abort.
pub type SolveResult = Result<(), String>;

/// Pascal `TSolutionObj` state.
pub struct Solution {
    pub mode: SolveMode,
    pub algorithm: i32,
    pub control_mode: i32,
    pub default_control_mode: i32,
    pub load_model: i32,
    pub default_load_model: i32,
    pub max_iterations: i32,
    pub min_iterations: i32,
    pub max_control_iterations: i32,
    pub convergence_tolerance: f64,
    pub converged_flag: bool,
    pub frequency: f64,
    pub harmonic: f64,
    pub frequency_changed: bool,
    pub system_y_changed: bool,
    pub series_y_invalid: bool,
    pub loads_need_updating: bool,
    pub voltage_base_changed: bool,
    pub solution_initialized: bool,
    pub use_aux_currents: bool,
    pub is_dynamic_model: bool,
    pub is_harmonic_model: bool,
    pub last_solution_was_direct: bool,
    pub solution_abort: bool,
    pub iteration: i32,
    pub most_iterations_done: i32,
    pub control_iteration: i32,
    pub control_actions_done: bool,
    pub max_error: f64,
    pub solution_count: i32,
    pub year: i32,
    pub dbl_hour: f64,
    pub interval_hrs: f64,
    pub number_of_times: i32,
    pub random_type: i32,
    pub sample_the_meters: bool,
    pub preserve_node_voltages: bool,

    /// `hYsystem` / `hYseries` / the `hY` selector.
    pub y_system: Option<SparseSet>,
    pub y_series: Option<SparseSet>,
    pub active_y: ActiveY,

    /// `NodeV[0..num_nodes]`, slot 0 = ground (always 0).
    pub node_v: Vec<Complex64>,
    /// `Currents[0..num_nodes]`, slot 0 absorbs ground injections.
    pub currents: Vec<Complex64>,
    pub aux_currents: Vec<Complex64>,
    /// `VMagSaved`/`ErrorSaved`/`NodeVbase`, 1-based with dummy slot 0.
    pub vmag_saved: Vec<f64>,
    pub error_saved: Vec<f64>,
    pub node_vbase: Vec<f64>,
    pub harmonic_list: Vec<f64>,

    /// `ckt.ControlQueue` — pending control actions (WP5.4). Driven by the
    /// control loop (WP5.7) once RegControl/CapControl `Sample` arms it.
    pub control_queue: ControlQueue,
    /// `DSS.EventStrings`, surfaced as `Solution.EventLog` to dss-python.
    pub event_log: EventLog,
}

impl Solution {
    /// Pascal `TSolutionObj.Create`.
    pub fn new(default_base_freq: f64) -> Self {
        Self {
            mode: SolveMode::Snapshot,
            algorithm: NORMALSOLVE,
            control_mode: CTRLSTATIC,
            default_control_mode: CTRLSTATIC,
            load_model: POWERFLOW,
            default_load_model: POWERFLOW,
            max_iterations: 15,
            min_iterations: 2,
            max_control_iterations: 10,
            convergence_tolerance: 0.0001,
            converged_flag: false,
            frequency: default_base_freq,
            harmonic: 1.0,
            frequency_changed: true,
            system_y_changed: true,
            series_y_invalid: true,
            loads_need_updating: true,
            voltage_base_changed: true,
            solution_initialized: false,
            use_aux_currents: false,
            is_dynamic_model: false,
            is_harmonic_model: false,
            last_solution_was_direct: false,
            solution_abort: false,
            iteration: 0,
            most_iterations_done: 0,
            control_iteration: 0,
            control_actions_done: false,
            max_error: 0.0,
            solution_count: 0,
            year: 0,
            dbl_hour: 0.0,
            interval_hrs: 1.0,
            number_of_times: 100,
            random_type: 1, // GAUSSIAN
            sample_the_meters: false,
            preserve_node_voltages: false,
            y_system: None,
            y_series: None,
            active_y: ActiveY::System,
            node_v: vec![Complex64::ZERO],
            currents: vec![Complex64::ZERO],
            aux_currents: Vec::new(),
            vmag_saved: vec![0.0],
            error_saved: vec![0.0],
            node_vbase: vec![0.0],
            harmonic_list: vec![1.0, 5.0, 7.0, 11.0, 13.0],
            control_queue: ControlQueue::new(),
            event_log: EventLog::new(),
        }
    }

    /// Pascal `Set_Mode`: reset times, revert control/load models, reset
    /// per-mode defaults. Phase 3 keeps the Snapshot/Direct paths.
    pub fn set_mode(&mut self, value: SolveMode) {
        self.dbl_hour = 0.0;
        self.mode = value;
        self.control_mode = self.default_control_mode;
        self.load_model = self.default_load_model;
        self.is_dynamic_model = false;
        self.is_harmonic_model = false;
        self.solution_initialized = false;
        self.preserve_node_voltages = false;
        self.sample_the_meters = false;
        if value == SolveMode::Snapshot {
            self.interval_hrs = 1.0;
            self.number_of_times = 1;
        }
    }

    /// Pascal `Set_Frequency`.
    pub fn set_frequency(&mut self, value: f64) {
        if self.frequency != value {
            self.frequency_changed = true;
            self.system_y_changed = true; // because of the frequency change
        }
        self.frequency = value;
        self.harmonic = 1.0; // TODO(phase7): Harmonic := Frequency / Fundamental
    }

    /// Pascal `ZeroInjCurr`.
    pub fn zero_inj_curr(&mut self) {
        self.currents.fill(Complex64::ZERO);
    }

    /// Pascal `SnapShotInit` (SetGeneratorDispRef is a no-op without
    /// generators in Phase 3).
    fn snap_shot_init(&mut self) {
        self.control_iteration = 0;
        self.control_actions_done = false;
        self.most_iterations_done = 0;
        self.loads_need_updating = true; // Force the loads to update at least once
    }

    /// The active sparse set (`hY`).
    fn active_sparse(&mut self) -> Result<&mut SparseSet, String> {
        let s = match self.active_y {
            ActiveY::System => self.y_system.as_mut(),
            ActiveY::Series => self.y_series.as_mut(),
        };
        s.ok_or_else(|| "System Y matrix not built yet".to_string())
    }

    /// Pascal `SolveSystem`: factor/solve `hY · NodeV[1..] = Currents[1..]`.
    pub fn solve_system(&mut self) -> SolveResult {
        let n = self.node_v.len() - 1;
        let b: Vec<Complex64> = self.currents[1..].to_vec();
        let mut x = vec![Complex64::ZERO; n];
        let sparse = self.active_sparse()?;
        sparse.factor().map_err(|e| {
            format!(
                "Error Solving System Y Matrix. Sparse matrix solver reports numerical error: {e}"
            )
        })?;
        sparse.solve(&b, &mut x).map_err(|e| {
            format!(
                "Error Solving System Y Matrix. Sparse matrix solver reports numerical error: {e}"
            )
        })?;
        self.node_v[1..].copy_from_slice(&x);
        Ok(())
    }

    /// Pascal `Converged`: per-node voltage-magnitude error against
    /// `NodeVbase` (or relative change when no base), exact NaN/Inf checks.
    fn converged(&mut self, num_nodes: usize) -> bool {
        self.max_error = 0.0;
        for i in 1..=num_nodes {
            let vmag = self.node_v[i].norm();
            if self.node_vbase[i] > 0.0 {
                self.error_saved[i] = (vmag - self.vmag_saved[i]).abs() / self.node_vbase[i];
            } else if vmag != 0.0 {
                self.error_saved[i] = (1.0 - self.vmag_saved[i] / vmag).abs();
            }
            self.vmag_saved[i] = vmag;
            self.max_error = self.max_error.max(self.error_saved[i]);
        }
        let result = self.max_error <= self.convergence_tolerance
            && !self.max_error.is_nan()
            && !self.max_error.is_infinite();
        self.converged_flag = result;
        result
    }
}

/// Snapshot of circuit/solution scalars for the elements (built fresh
/// whenever flags may have changed).
pub fn sys_ctx(ckt: &Circuit) -> SysCtx {
    let s = &ckt.solution;
    SysCtx {
        frequency: s.frequency,
        fundamental: ckt.fundamental,
        is_harmonic_model: s.is_harmonic_model,
        is_dynamic_model: s.is_dynamic_model,
        load_model: s.load_model,
        mode: s.mode,
        load_multiplier: ckt.load_multiplier,
        default_growth_factor: ckt.default_growth_factor,
        year: s.year,
        dbl_hour: s.dbl_hour,
        solution_count: s.solution_count,
        loads_need_updating: s.loads_need_updating,
        neglect_load_y: ckt.neglect_load_y,
        long_line_correction: ckt.long_line_correction,
        positive_sequence: ckt.positive_sequence,
    }
}

/// Pascal `GetSourceInjCurrents`: all enabled sources inject into `Currents`.
/// (The GFM PCE pass is empty in Phase 3.)
fn get_source_inj_currents(ckt: &mut Circuit, env: &mut SolveEnv) {
    let sys = sys_ctx(ckt);
    let sol = &mut ckt.solution;
    let mut ctx = InjCtx {
        node_v: &sol.node_v,
        currents: &mut sol.currents,
    };
    for &r in &ckt.sources {
        let elem = env.store.ckt_elem_mut(r);
        if elem.cd().enabled {
            elem.inj_currents(&sys, &mut ctx);
        }
    }
}

/// Pascal `GetPCInjCurr`: all enabled PC elements inject into `Currents`.
fn get_pc_inj_curr(ckt: &mut Circuit, env: &mut SolveEnv) {
    let sys = sys_ctx(ckt);
    let sol = &mut ckt.solution;
    let mut ctx = InjCtx {
        node_v: &sol.node_v,
        currents: &mut sol.currents,
    };
    for &r in &ckt.pc_elements {
        let elem = env.store.ckt_elem_mut(r);
        if elem.cd().enabled {
            elem.inj_currents(&sys, &mut ctx);
        }
    }
}

/// Pascal `DoNormalSolution`: the fixed-point loop.
fn do_normal_solution(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.iteration = 0;
    loop {
        ckt.solution.iteration += 1;

        ckt.solution.zero_inj_curr();
        get_source_inj_currents(ckt, env);
        get_pc_inj_curr(ckt, env);

        // The above calls could change the primitive Y matrix, so check.
        if ckt.solution.system_y_changed {
            build_y_matrix(ckt, env, BuildOption::WholeMatrix, false)?;
        }
        // UseAuxCurrents/AddInAuxCurrents: AutoAdd only, not in Phase 3.

        ckt.solution.solve_system()?;
        ckt.solution.loads_need_updating = false;

        let num_nodes = ckt.num_nodes;
        let converged = ckt.solution.converged(num_nodes);
        if (converged && ckt.solution.iteration >= ckt.solution.min_iterations)
            || ckt.solution.iteration >= ckt.solution.max_iterations
        {
            return Ok(());
        }
    }
}

/// Pascal `SolveYDirect`: solve with only source injections.
fn solve_y_direct(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.zero_inj_curr();
    get_source_inj_currents(ckt, env);
    if ckt.solution.is_dynamic_model {
        get_pc_inj_curr(ckt, env);
    }
    ckt.solution.solve_system()
}

/// Pascal `SolveZeroLoadSnapShot`: series-only solve for initialization.
pub fn solve_zero_load_snapshot(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    if ckt.solution.system_y_changed || ckt.solution.series_y_invalid {
        build_y_matrix(ckt, env, BuildOption::SeriesOnly, true)?; // Side effect: allocates V
    }
    ckt.solution.solution_count += 1;
    ckt.solution.zero_inj_curr();
    get_source_inj_currents(ckt, env);

    // Make the series Y matrix the active matrix.
    if ckt.solution.y_series.is_none() {
        return Err("Series Y matrix not built yet in SolveZeroLoadSnapshot.".to_string());
    }
    ckt.solution.active_y = ActiveY::Series;
    let result = ckt.solution.solve_system();

    // Reset the main system Y as the solution matrix.
    if ckt.solution.y_system.is_some() {
        ckt.solution.active_y = ActiveY::System;
    }
    result
}

/// Pascal `DoPFLOWsolution`.
fn do_pflow_solution(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.solution_count += 1;

    if ckt.solution.voltage_base_changed {
        initialize_node_vbase(ckt); // for convergence test
    }

    if !ckt.solution.solution_initialized {
        // "8-14-06 This should give a better answer than zero load snapshot"
        solve_y_direct(ckt, env)?;
        // SetGeneratordQdV: no Model-3 generators in Phase 3 → no extra work.
        ckt.solution.solution_initialized = true;
    }

    match ckt.solution.algorithm {
        NEWTONSOLVE => Err("Newton solution not ported in Phase 3".to_string()),
        _ => do_normal_solution(ckt, env),
    }?;

    ckt.is_solved = ckt.solution.converged_flag;
    ckt.solution.last_solution_was_direct = false;
    Ok(())
}

/// Pascal `SolveCircuit`.
fn solve_circuit(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    if ckt.solution.load_model == ADMITTANCE {
        solve_direct(ckt, env)
    } else {
        if ckt.solution.system_y_changed {
            build_y_matrix(ckt, env, BuildOption::WholeMatrix, true)?; // Side effect: allocates V
        }
        do_pflow_solution(ckt, env)
    }
}

/// Pascal `Sample_DoControlActions` + `CheckControls` for a circuit without
/// control elements: an empty control queue means actions are done.
fn check_controls(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    let sol = &mut ckt.solution;
    if sol.control_iteration < sol.max_control_iterations {
        if sol.converged_flag {
            if sol.control_mode == CONTROLSOFF {
                sol.control_actions_done = true;
            } else {
                // SampleControlDevices + DoControlActions over an empty
                // control list / queue (Phase 5 ports the real thing).
                sol.control_actions_done = true;
            }
        } else {
            sol.control_actions_done = true; // Stop if failure to converge
        }
    }
    if ckt.solution.system_y_changed {
        build_y_matrix(ckt, env, BuildOption::WholeMatrix, false)?;
    }
    Ok(())
}

/// Pascal `SolveSnap`.
fn solve_snap(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.snap_shot_init();
    let mut total_iterations = 0;
    loop {
        ckt.solution.control_iteration += 1;

        solve_circuit(ckt, env)?; // Do circuit solution w/o checking controls
        check_controls(ckt, env)?;

        // For reporting max iterations per control iteration
        if ckt.solution.iteration > ckt.solution.most_iterations_done {
            ckt.solution.most_iterations_done = ckt.solution.iteration;
        }
        total_iterations += ckt.solution.iteration;

        if ckt.solution.control_actions_done
            || ckt.solution.control_iteration >= ckt.solution.max_control_iterations
        {
            break;
        }
    }

    if !ckt.solution.control_actions_done
        && ckt.solution.control_iteration >= ckt.solution.max_control_iterations
    {
        env.errors
            .push("Warning Max Control Iterations Exceeded.".to_string());
        ckt.solution.solution_abort = true;
    }

    ckt.solution.iteration = total_iterations; // "so that it reports a more interesting number"
    Ok(())
}

/// Pascal `SolveDirect`.
fn solve_direct(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.solution.loads_need_updating = true;
    ckt.solution.solution_count += 1;

    if ckt.solution.system_y_changed {
        build_y_matrix(ckt, env, BuildOption::WholeMatrix, true)?;
    }
    ckt.solution.zero_inj_curr();
    get_source_inj_currents(ckt, env);
    if ckt.solution.is_dynamic_model || ckt.solution.is_harmonic_model {
        get_pc_inj_curr(ckt, env);
    }
    ckt.solution.solve_system()?;
    ckt.is_solved = true;
    ckt.solution.converged_flag = true;
    ckt.solution.iteration = 1;
    ckt.solution.last_solution_was_direct = true;
    Ok(())
}

/// Pascal `TSolutionObj.Solve`: the mode dispatcher (Phase 3: Snapshot and
/// Direct; the rest report error 481/482 style messages).
pub fn solve(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    ckt.is_solved = false;
    ckt.solution_was_attempted = true;

    if ckt.emerg_min_volts >= ckt.normal_min_volts {
        env.errors.push(
            "Error: Emergency Min Voltage Must Be Less Than Normal Min Voltage! Solution Not Executed."
                .to_string(),
        );
        return Ok(());
    }
    if ckt.solution.solution_abort {
        env.errors.push("Solution aborted.".to_string());
        return Ok(());
    }

    ckt.default_growth_factor = if ckt.solution.year == 0 {
        1.0
    } else {
        ckt.default_growth_rate.powi(ckt.solution.year - 1)
    };

    let result = match ckt.solution.mode {
        SolveMode::Snapshot => solve_snap(ckt, env),
        SolveMode::Direct => solve_direct(ckt, env),
        _ => {
            env.errors
                .push("Unknown solution mode.".to_string() + " (not ported in Phase 3)");
            Ok(())
        }
    };
    if let Err(e) = &result {
        env.errors.push(format!("Error Encountered in Solve: {e}"));
        ckt.solution.solution_abort = true;
    }
    Ok(())
}

/// Pascal `nearestBasekV`.
fn nearest_base_kv(ckt: &Circuit, kv: f64) -> f64 {
    let mut result = 0.0;
    let mut min_diff = 1.0e50;
    for &test_kv in &ckt.legal_voltage_bases {
        if test_kv == 0.0 {
            break;
        }
        let diff = (1.0 - kv / test_kv).abs();
        if diff < min_diff {
            min_diff = diff;
            result = test_kv;
        }
    }
    result
}

/// Pascal `SetVoltageBases` (the `CalcVoltageBases` command): zero-load
/// solve, then assign each bus the nearest legal base.
pub fn set_voltage_bases(ckt: &mut Circuit, env: &mut SolveEnv) -> SolveResult {
    // Meter zones are not built in this load flow (no meters in Phase 3).
    solve_zero_load_snapshot(ckt, env)?;

    for b in 0..ckt.buses.len() {
        // TODO(compat): Pascal scales |V| by the literal 0.001732 (truncated
        // √3/1000) before the nearest-base search, then divides the matched
        // base by the full-precision SQRT3. Reproduced exactly; clean fix is
        // a single constant after final acceptance.
        let vref = ckt.buses[b].get_ref(0);
        let kv_est = ckt.solution.node_v[vref].norm() * 0.001732;
        ckt.buses[b].kv_base = nearest_base_kv(ckt, kv_est) / sqrt3();
    }

    initialize_node_vbase(ckt); // for convergence test
    ckt.is_solved = true;
    Ok(())
}
