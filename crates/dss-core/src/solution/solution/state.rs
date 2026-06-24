//! The solution state: the [`SolveMode`]/[`ActiveY`] enums, the
//! load-model/algorithm/control-mode constants, the [`SolveEnv`] disjoint-borrow
//! context, the [`Solution`] (`TSolutionObj`) state struct with its clock,
//! injection-vector and `SolveSystem`/`Converged` helpers, and [`sys_ctx`].

use num_complex::Complex64;

use dss_parser::{Parser, ParserVars};
use dss_sparse::SparseSet;

use crate::circuit::Circuit;
use crate::elements::traits::{ElemStore, SysCtx};
use crate::solution::control_queue::ControlQueue;
use crate::solution::event_log::EventLog;

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
    /// `DynaVars.intHour` / `.t` (seconds into the hour) / `.h` (step size in
    /// seconds) / `.dblHour` — the Phase 1 `TDynamicsRec` fields the solution
    /// actually drives (PHASE5_PLAN §WP5.8).
    pub int_hour: i32,
    pub t: f64,
    pub h: f64,
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
            int_hour: 0,
            t: 0.0,
            h: 0.001, // default for dynasolve
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

    /// Pascal `Update_dblHour`: `dblHour = intHour + t/3600`.
    pub fn update_dbl_hour(&mut self) {
        self.dbl_hour = self.int_hour as f64 + self.t / 3600.0;
    }

    /// Pascal `TSolutionObj.TimeOfDay(useEpsilon=false)`: the hour `intHour`
    /// wrapped into `0..24` plus `t/3600`, wrapped once more past 24:00.
    pub fn time_of_day(&self) -> f64 {
        let hour_of_day = if self.int_hour > 23 {
            self.int_hour - (self.int_hour / 24) * 24
        } else {
            self.int_hour
        };
        let r = hour_of_day as f64 + self.t / 3600.0;
        if r > 24.0 { r - 24.0 } else { r }
    }

    /// Pascal `IncrementTime`: `t += h`, rolling whole hours into `intHour`.
    pub fn increment_time(&mut self) {
        self.t += self.h;
        while self.t >= 3600.0 {
            self.int_hour += 1;
            self.t -= 3600.0;
        }
        self.update_dbl_hour();
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
    pub(super) fn snap_shot_init(&mut self) {
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

    /// KLUSolve `GetMatrixElement` on the assembled system Y (`hYsystem`):
    /// 1-based global node `node` maps to matrix index `node - 1`.
    pub fn system_matrix_element(&mut self, node: usize) -> Result<Complex64, String> {
        let y = self
            .y_system
            .as_mut()
            .ok_or_else(|| "System Y matrix not built yet".to_string())?;
        let i = node - 1;
        y.get_element(i, i).map_err(|e| e.to_string())
    }

    /// Pascal `Converged`: per-node voltage-magnitude error against
    /// `NodeVbase` (or relative change when no base), exact NaN/Inf checks.
    pub(super) fn converged(&mut self, num_nodes: usize) -> bool {
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
        gen_multiplier: ckt.gen_multiplier,
        generator_dispatch_reference: ckt.generator_dispatch_reference,
        price_signal: ckt.price_signal,
        default_growth_factor: ckt.default_growth_factor,
        year: s.year,
        dbl_hour: s.dbl_hour,
        solution_count: s.solution_count,
        loads_need_updating: s.loads_need_updating,
        neglect_load_y: ckt.neglect_load_y,
        long_line_correction: ckt.long_line_correction,
        positive_sequence: ckt.positive_sequence,
        time_of_day: s.time_of_day(),
        dyna_h: s.h,
    }
}
