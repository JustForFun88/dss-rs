//! The solution state: the [`SolveMode`]/[`ActiveY`] enums, the
//! load-model/algorithm/control-mode constants, the [`SolveEnv`] disjoint-borrow
//! context, the [`Solution`] (`TSolutionObj`) state struct with its clock,
//! injection-vector and `SolveSystem`/`Converged` helpers, and [`sys_ctx`].

use num_complex::Complex64;

use dss_parser::{Parser, ParserVars};
use dss_sparse::{RealSparseSet, SparseSet};

use crate::circuit::Circuit;
use crate::elements::traits::{ElemStore, SysCtx};
use crate::solution::control_queue::ControlQueue;
use crate::solution::event_log::EventLog;
use crate::support::dynamics::IterationFlag;

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

/// Load model codes (`DSSGlobals.pas:90-91` — `POWERFLOW = 1`,
/// `ADMITTANCE = 2`), the `Set LoadModel=` option / `Solution.LoadModel` field.
/// Discriminants are user-visible and frozen (they round-trip through the
/// `Load Solution Model` `DssEnum`, values `[1, 2]`); `i32` survives only at the
/// parse/report boundary. Named `LoadSolutionModel` to avoid colliding with the
/// Load element's own `LoadModel` (`Model=` property, ConstPQ..).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum LoadSolutionModel {
    #[default]
    PowerFlow = 1,
    Admittance = 2,
}

impl LoadSolutionModel {
    /// The `Load Solution Model` `DssEnum` ordinal (`Set loadmodel=`/`?`/dump).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// From the enum-registry value; out-of-range yields `None`.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            1 => Some(Self::PowerFlow),
            2 => Some(Self::Admittance),
            _ => None,
        }
    }
}

/// Random distribution codes (`DSSGlobals.pas:106-108` — `GAUSSIAN = 1`,
/// `UNIFORM = 2`, `LOGNORMAL = 3`; `none` is the registry's `0`):
/// `Solution.RandomType` (`Set random=`, `RandomModeEnum` values
/// `[0, 1, 2, 3]`). Consumed by the MonteCarlo `Randomize` paths, where every
/// non-listed code (i.e. `None`) leaves the multiplier at 1.0.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum RandomType {
    None = 0,
    /// `TSolutionObj.Create` seeds `RandomType := GAUSSIAN`
    /// (`Solution.pas:487`), so `Gaussian` — not the numerically-zero `None` —
    /// is this family's `Default`, matching every other enum in this wave
    /// (`default()` == the Pascal `Create` value). Settler note (2026-07-26):
    /// the derive originally fell on `None`, which would have silently turned
    /// `Set random` off had anything ever reached for `unwrap_or_default()`
    /// here; nothing did, so the change is bit-neutral, and the pin test now
    /// asserts it.
    #[default]
    Gaussian = 1,
    Uniform = 2,
    LogNormal = 3,
}

impl RandomType {
    /// The `Random Type` `DssEnum` ordinal (`Set random=`/`?`/dump boundary).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// From the enum-registry value; out-of-range yields `None`.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::Gaussian),
            2 => Some(Self::Uniform),
            3 => Some(Self::LogNormal),
            _ => None,
        }
    }
}

/// Load-shape class codes (`DSSGlobals.pas`): the class the GENERALTIME /
/// DYNAMICMODE dispatch picks (`Circuit.ActiveLoadShapeClass`, `Set
/// LoadShapeClass=`). `USENONE` (-1) = not set → `ShapeFactor = 1+j1`.
pub const USEDAILY: i32 = 0;
pub const USEYEARLY: i32 = 1;
pub const USEDUTY: i32 = 2;
pub const USENONE: i32 = -1;

/// Power-flow algorithm (`Solution.pas` l.39-41, `SolveAlgEnum`: `Set
/// algorithm=`). Discriminants are user-visible and frozen (round-trip through
/// the `DssEnum` registry); `i32` survives only at the parse/report boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum SolveAlgorithm {
    #[default]
    Normal = 0,
    Newton = 1,
    Ncim = 2,
}

impl SolveAlgorithm {
    /// The `SolveAlgEnum` ordinal (`Set algorithm=`/`?`/dump boundary value).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// `TSolveAlgorithm(ordinal)` from the enum-registry value; out-of-range
    /// yields `None`.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Normal),
            1 => Some(Self::Newton),
            2 => Some(Self::Ncim),
            _ => None,
        }
    }
}

/// NCIM node-type codes (`Solution.pas` l.44-45).
pub const NCIM_PQ_NODE: i32 = 0;
pub const NCIM_PV_NODE: i32 = 1;

/// Control modes (`DSSGlobals.pas:99-103` — `CONTROLSOFF = -1`,
/// `CTRLSTATIC = 0`, `EVENTDRIVEN = 1`, `TIMEDRIVEN = 2`, `MULTIRATE = 3`):
/// `Solution.ControlMode` / `.DefaultControlMode` (`Set controlmode=`). The
/// discriminants are user-visible and frozen (they round-trip through the
/// `Control Mode` `DssEnum`, values `[-1, 0, 1, 2, 3]`); `i32` survives only at
/// the parse/report boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum ControlMode {
    /// `CONTROLSOFF` — controls disabled (`Set controlmode=Off`).
    ControlsOff = -1,
    /// `CTRLSTATIC` — the default: one control action set per solution.
    #[default]
    Static = 0,
    /// `EVENTDRIVEN`.
    EventDriven = 1,
    /// `TIMEDRIVEN`.
    TimeDriven = 2,
    /// `MULTIRATE`.
    MultiRate = 3,
}

impl ControlMode {
    /// The `Control Mode` `DssEnum` ordinal (`Set controlmode=`/`?`/dump).
    pub fn ordinal(self) -> i32 {
        self as i32
    }

    /// From the enum-registry value; out-of-range yields `None`.
    pub fn from_ordinal(value: i32) -> Option<Self> {
        match value {
            -1 => Some(Self::ControlsOff),
            0 => Some(Self::Static),
            1 => Some(Self::EventDriven),
            2 => Some(Self::TimeDriven),
            3 => Some(Self::MultiRate),
            _ => None,
        }
    }
}

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
    pub errors: &'a mut crate::diag::ErrorLog,
}

/// `ESolveError`-style hard abort.
pub type SolveResult = Result<(), String>;

/// Pascal `TSolutionObj` state.
pub struct Solution {
    pub mode: SolveMode,
    pub algorithm: SolveAlgorithm,
    pub control_mode: ControlMode,
    pub default_control_mode: ControlMode,
    pub load_model: LoadSolutionModel,
    pub default_load_model: LoadSolutionModel,
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
    /// Solve timers in microseconds (Pascal `Solve_Time_Elapsed` /
    /// `Total_Time_Elapsed` / `Step_Time_Elapsed`, `Solution.pas:211-214`),
    /// surfaced by `Get`/`Set processtime|totaltime|steptime`. Upstream fills
    /// these from `QueryPerformanceCounter` — inherently non-deterministic
    /// wall-clock, which this port does **not** reproduce (same convention as
    /// the monitor time channels 11/12 hardcoded to 0, `monitor/mod.rs:136`):
    /// they stay `0.0`, and only `total_time_elapsed` is user-settable
    /// (`set totaltime=…`). The gate-able surface is the round-trip
    /// (fresh/after-reset → 0; after `set totaltime=v` → v); any post-solve
    /// timing value is non-deterministic and never gated.
    pub solve_time_elapsed: f64,
    pub total_time_elapsed: f64,
    pub step_time_elapsed: f64,
    /// `DynaVars.IterationFlag`: predictor (`NewTimeStep`) vs corrector
    /// (`SameTimeStep`) within a dynamics time step (`SolveDynamic`).
    pub iteration_flag: IterationFlag,
    pub interval_hrs: f64,
    pub number_of_times: i32,
    pub random_type: RandomType,
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
    /// Reusable right-hand-side scratch for [`Solution::solve_system_into`] — the
    /// `Currents[1..]` copy the sparse solve consumes. Kept as a field so the
    /// per-fixed-point-iteration solve does not allocate a fresh vector each call
    /// (DE_PASCALIZE P15 item 5); `mem::take`-swapped in and out to split the
    /// borrow against the active sparse set.
    solve_rhs: Vec<Complex64>,
    /// `VMagSaved`/`ErrorSaved`/`NodeVbase`, 1-based with dummy slot 0.
    pub vmag_saved: Vec<f64>,
    pub error_saved: Vec<f64>,
    pub node_vbase: Vec<f64>,
    /// Pascal `savePresentVoltages`/`RetrieveSavedVoltages`: the fundamental
    /// `NodeV` captured when entering harmonics mode (the original spills it to a
    /// `SavedVoltages.dbl` file; this port keeps it in memory). Restored before a
    /// harmonic sweep if the last solution was off-fundamental.
    pub saved_node_v: Vec<Complex64>,
    pub harmonic_list: Vec<f64>,
    /// Pascal `DoAllHarmonics`: when true the harmonic sweep collects every
    /// frequency in use across the spectra; otherwise it uses `harmonic_list`.
    pub do_all_harmonics: bool,

    /// `ckt.ControlQueue` — pending control actions (WP5.4). Driven by the
    /// control loop (WP5.7) once RegControl/CapControl `Sample` arms it.
    pub control_queue: ControlQueue,
    /// `DSS.EventStrings`, surfaced as `Solution.EventLog` to dss-python.
    pub event_log: EventLog,
    /// Branch-to-node incidence matrix + Laplacian (Pascal `IncMat`/`Laplacian`/
    /// `Inc_Mat_Rows`/`Inc_Mat_Cols`/`Inc_Mat_levels`), built by the
    /// `CalcIncMatrix`/`CalcIncMatrix_O`/`CalcLaplacian` commands (WP-AD.1).
    pub inc_matrix: crate::solution::inc_matrix::IncMatrixState,

    // --- A-Diakoptics (WP-AD.3, `Solution.pas:233–255`). All false/empty until
    // `set ADiakoptics=yes` runs `ADiakopticsInit`. On the coordinator these
    // steer the solve-path AD branches; on a child, the index maps route its
    // local solve into the coordinator's NodeV.
    /// `Solution.ADiakoptics` — the AD-active flag (coordinator).
    pub adiakoptics: bool,
    /// `Solution.ADiak_Init` — set TRUE at the tail of `ADiakopticsInit` state 9,
    /// then cleared on success ("force the subzones to remove VSource.Source")
    /// and reset by every `Solve` (Diakoptics.pas:748/770, ExecCommands `Solve`).
    pub adiak_init: bool,
    /// `Solution.ADiak_PCInj` — TRUE inside `DoNormalSolution`'s coordinator hook
    /// (children pick up PC injections), FALSE from `SolveDirect`/`SolveYDirect`.
    pub adiak_pcinj: bool,
    /// `Parallel_enabled` (a Pascal global; sequential here). TRUE once
    /// `ADiakopticsInit` succeeds.
    pub parallel_enabled: bool,
    /// `LocalBusIdx` (child): the coordinator node index of each of this child's
    /// nodes; `LocalBusIdx[0]` is the child's offset into the parent NodeV.
    pub local_bus_idx: Vec<usize>,
    /// `AD_IBus` (child): local node indices (1-based) carrying an AD current
    /// injection (a boundary bus present in this child's `Contours` rows).
    pub ad_ibus: Vec<usize>,
    /// `AD_ISrcIdx` (child): the coordinator `Contours` row each `AD_IBus` entry
    /// maps to (the `Ic` row read in `UpdateISrc`).
    pub ad_isrc_idx: Vec<i32>,

    // --- NCIM solver state (`Solution.pas` l.243-271). All empty/false until
    // `Set Algorithm=NCIM` runs `DoNCIMSolution`. The 1-based-with-dummy-slot-0
    // vectors mirror the Pascal `array of` layout (index 0 is the ground
    // placeholder — `NCIM_NodeType[0] = -1` "ignore"), so a node `i` (1-based,
    // ground=0) indexes `NCIM_*[i]` directly. The Jacobian uses its own 0-based
    // layout: node `i` (1-based) → Jacobian rows `2*(i-1)`, `2*(i-1)+1`.
    /// `NCIM_deltaZ` — voltage-mismatch solution vector (the solve output).
    pub ncim_delta_z: Vec<f64>,
    /// `NCIM_deltaF` — injection-current mismatch vector (the solve RHS).
    pub ncim_delta_f: Vec<f64>,
    /// `NCIM_NodePower` — total complex power per node.
    pub ncim_node_power: Vec<Complex64>,
    /// `NCIM_GenPower` — total generation power per node (per iteration).
    pub ncim_gen_power: Vec<Complex64>,
    /// `NCIM_Y` — nonzero values of the PDE-only Y bus (triplet form).
    pub ncim_y: Vec<Complex64>,
    /// `NCIM_NodeLimits` — total Q limits per node (re=qMax, im=qMin), for PV buses.
    pub ncim_node_limits: Vec<Complex64>,
    /// `NCIM_YRow` — row indices (0-based) of the `NCIM_Y` nonzeros.
    pub ncim_y_row: Vec<usize>,
    /// `NCIM_YCol` — col indices (0-based) of the `NCIM_Y` nonzeros.
    pub ncim_y_col: Vec<usize>,
    /// `NCIM_NodeType` — per-node PQ (0) / PV (1) classification (slot 0 = -1).
    pub ncim_node_type: Vec<i32>,
    /// `NCIM_NodeNumGen` — number of generators per node.
    pub ncim_node_num_gen: Vec<i32>,
    /// `NCIM_NodePVTarget` — target (voltage) of the PV buses.
    pub ncim_node_pv_target: Vec<f64>,
    /// `NCIM_PVBusIdx` — the PV-bus current index within the Jacobian's gen rows.
    pub ncim_pv_bus_idx: Vec<i32>,
    /// `NCIM_Jacobian` — the real-valued sparse Jacobian (rebuilt each iteration).
    pub ncim_jacobian: Option<RealSparseSet>,
    /// `NCIM_InitGenQ` — first-run / reinit flag for the generator Q registries.
    pub ncim_init_gen_q: bool,
    /// `NCIM_Ready` — whether the NCIM structures are initialized.
    pub ncim_ready: bool,
    /// `NCIM_IgnoreQLimit` — `Set IgnoreGenQLimits`.
    pub ncim_ignore_q_limit: bool,
    /// `NCIM_GenGain` — `Set NCIMQGain` (global reactive-power injection gain).
    pub ncim_gen_gain: f64,
    /// `NCIM_Nodes` — number of nodes in the PDE-only Y bus.
    pub ncim_nodes: usize,
}

impl Solution {
    /// Pascal `TSolutionObj.Create`.
    pub fn new(default_base_freq: f64) -> Self {
        Self {
            mode: SolveMode::Snapshot,
            algorithm: SolveAlgorithm::Normal,
            control_mode: ControlMode::Static,
            default_control_mode: ControlMode::Static,
            load_model: LoadSolutionModel::PowerFlow,
            default_load_model: LoadSolutionModel::PowerFlow,
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
            solve_time_elapsed: 0.0,
            total_time_elapsed: 0.0,
            step_time_elapsed: 0.0,
            iteration_flag: IterationFlag::NewTimeStep,
            interval_hrs: 1.0,
            number_of_times: 100,
            random_type: RandomType::Gaussian,
            sample_the_meters: false,
            preserve_node_voltages: false,
            y_system: None,
            y_series: None,
            active_y: ActiveY::System,
            node_v: vec![Complex64::ZERO],
            currents: vec![Complex64::ZERO],
            aux_currents: Vec::new(),
            solve_rhs: Vec::new(),
            vmag_saved: vec![0.0],
            error_saved: vec![0.0],
            node_vbase: vec![0.0],
            saved_node_v: Vec::new(),
            harmonic_list: vec![1.0, 5.0, 7.0, 11.0, 13.0],
            do_all_harmonics: true,
            control_queue: ControlQueue::new(),
            event_log: EventLog::new(),
            inc_matrix: crate::solution::inc_matrix::IncMatrixState::default(),
            adiakoptics: false,
            adiak_init: false,
            adiak_pcinj: false,
            parallel_enabled: false,
            local_bus_idx: Vec::new(),
            ad_ibus: Vec::new(),
            ad_isrc_idx: Vec::new(),
            ncim_delta_z: Vec::new(),
            ncim_delta_f: Vec::new(),
            ncim_node_power: Vec::new(),
            ncim_gen_power: Vec::new(),
            ncim_y: Vec::new(),
            ncim_node_limits: Vec::new(),
            ncim_y_row: Vec::new(),
            ncim_y_col: Vec::new(),
            ncim_node_type: Vec::new(),
            ncim_node_num_gen: Vec::new(),
            ncim_node_pv_target: Vec::new(),
            ncim_pv_bus_idx: Vec::new(),
            ncim_jacobian: None,
            ncim_init_gen_q: true,
            ncim_ready: false,
            ncim_ignore_q_limit: false,
            ncim_gen_gain: 1.0,
            ncim_nodes: 0,
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

    /// Pascal `Set_Frequency`. `fundamental` is the circuit base frequency
    /// (`ActiveCircuit.Fundamental`), which `Solution` does not own.
    pub fn set_frequency(&mut self, value: f64, fundamental: f64) {
        if self.frequency != value {
            self.frequency_changed = true;
            self.system_y_changed = true; // because of the frequency change
        }
        self.frequency = value;
        self.harmonic = value / fundamental;
    }

    /// Pascal `ZeroInjCurr`.
    pub fn zero_inj_curr(&mut self) {
        self.currents.fill(Complex64::ZERO);
    }

    /// Pascal `SnapShotInit` (SetGeneratorDispRef is a no-op without
    /// generators in Phase 3).
    pub(crate) fn snap_shot_init(&mut self) {
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

    /// Pascal `SolveSystem(V)`: factor/solve `hY · x[1..] = Currents[1..]` into
    /// the caller buffer `out` (length `NumNodes`, 0-based; `out[k]` is global
    /// node `k+1`). `DoNormalSolution` passes `NodeV`; `DoNewtonSolution` passes
    /// the delta-V work array `dV`.
    pub(crate) fn solve_system_into(&mut self, out: &mut [Complex64]) -> SolveResult {
        // Reuse the RHS scratch field (P15 item 5). `mem::take` swaps its buffer
        // out so the `&mut self` borrow the active sparse set needs does not
        // alias it; the buffer (and its capacity) is put back before returning.
        let mut b = std::mem::take(&mut self.solve_rhs);
        b.clear();
        b.extend_from_slice(&self.currents[1..]);
        let result = (|| {
            let sparse = self.active_sparse()?;
            sparse.factor().map_err(|e| {
                format!(
                    "Error Solving System Y Matrix. Sparse matrix solver reports numerical error: {e}"
                )
            })?;
            sparse.solve(&b, out).map_err(|e| {
                format!(
                    "Error Solving System Y Matrix. Sparse matrix solver reports numerical error: {e}"
                )
            })?;
            Ok(())
        })();
        self.solve_rhs = b;
        result
    }

    /// Pascal `SolveSystem(NodeV)`: solve directly into the node-voltage array.
    pub fn solve_system(&mut self) -> SolveResult {
        let n = self.node_v.len() - 1;
        let mut x = vec![Complex64::ZERO; n];
        self.solve_system_into(&mut x)?;
        self.node_v[1..].copy_from_slice(&x);
        Ok(())
    }

    /// Pascal `DoNewtonSolution`'s `SolveSystem(dV); NodeV[i] -= dV[i]`: solve
    /// for the voltage delta into a scratch `dV` and subtract it from `NodeV`.
    pub(super) fn solve_system_newton_step(&mut self) -> SolveResult {
        let num_nodes = self.node_v.len() - 1;
        let mut dv = vec![Complex64::ZERO; num_nodes];
        self.solve_system_into(&mut dv)?;
        for i in 1..=num_nodes {
            self.node_v[i] -= dv[i - 1];
        }
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
    pub(crate) fn converged(&mut self, num_nodes: usize) -> bool {
        // Pascal `Converged` (`Solution.pas` l.731-734): NCIM has its own test.
        if self.algorithm == SolveAlgorithm::Ncim {
            return self.ncim_converged();
        }
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

    /// Pascal `TNCIMSolutionHelper.NCIM_Converged` (`NCIMSolutionHelper.pas`
    /// l.1034): converged when every injection-current mismatch `|deltaF[i]|` is
    /// within `ConvergenceTolerance`. The Pascal seeds `Result := false` (so an
    /// empty `deltaF` reports NOT converged) and breaks on the first miss.
    fn ncim_converged(&mut self) -> bool {
        let mut result = false;
        for &f in &self.ncim_delta_f {
            result = f.abs() <= self.convergence_tolerance;
            if !result {
                break;
            }
        }
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
        active_load_shape_class: ckt.active_load_shape_class,
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
        dyna_t: s.t,
        iteration_flag: s.iteration_flag,
        last_solution_was_direct: s.last_solution_was_direct,
    }
}

#[cfg(test)]
mod enum_discriminant_tests {
    use super::{ControlMode, LoadSolutionModel, RandomType, SolveAlgorithm};

    #[test]
    fn solve_algorithm_pins_solvealgenum_ordinals() {
        assert_eq!(SolveAlgorithm::Normal.ordinal(), 0);
        assert_eq!(SolveAlgorithm::Newton.ordinal(), 1);
        assert_eq!(SolveAlgorithm::Ncim.ordinal(), 2);
        for (ord, alg) in [
            (0, SolveAlgorithm::Normal),
            (1, SolveAlgorithm::Newton),
            (2, SolveAlgorithm::Ncim),
        ] {
            assert_eq!(SolveAlgorithm::from_ordinal(ord), Some(alg));
        }
        assert_eq!(SolveAlgorithm::from_ordinal(-1), None);
        assert_eq!(SolveAlgorithm::from_ordinal(3), None);
    }

    /// `DSSGlobals.pas:99-103` + the `Control Mode` `DssEnum`
    /// (`registry/solution.rs`, values `[-1, 0, 1, 2, 3]`).
    #[test]
    fn control_mode_pins_enum_ordinals() {
        assert_eq!(ControlMode::ControlsOff.ordinal(), -1);
        assert_eq!(ControlMode::Static.ordinal(), 0);
        assert_eq!(ControlMode::EventDriven.ordinal(), 1);
        assert_eq!(ControlMode::TimeDriven.ordinal(), 2);
        assert_eq!(ControlMode::MultiRate.ordinal(), 3);
        for m in [
            ControlMode::ControlsOff,
            ControlMode::Static,
            ControlMode::EventDriven,
            ControlMode::TimeDriven,
            ControlMode::MultiRate,
        ] {
            assert_eq!(ControlMode::from_ordinal(m.ordinal()), Some(m));
        }
        assert_eq!(ControlMode::from_ordinal(-2), None);
        assert_eq!(ControlMode::from_ordinal(4), None);
        // `TSolutionObj.Create` default is CTRLSTATIC.
        assert_eq!(ControlMode::default(), ControlMode::Static);
    }

    /// `DSSGlobals.pas:90-91` + the `Load Solution Model` `DssEnum` (`[1, 2]`).
    #[test]
    fn load_solution_model_pins_enum_ordinals() {
        assert_eq!(LoadSolutionModel::PowerFlow.ordinal(), 1);
        assert_eq!(LoadSolutionModel::Admittance.ordinal(), 2);
        for m in [LoadSolutionModel::PowerFlow, LoadSolutionModel::Admittance] {
            assert_eq!(LoadSolutionModel::from_ordinal(m.ordinal()), Some(m));
        }
        assert_eq!(LoadSolutionModel::from_ordinal(0), None);
        assert_eq!(LoadSolutionModel::from_ordinal(3), None);
        assert_eq!(LoadSolutionModel::default(), LoadSolutionModel::PowerFlow);
    }

    /// `DSSGlobals.pas:106-108` + the `Random Type` `DssEnum` (`[0, 1, 2, 3]`).
    #[test]
    fn random_type_pins_enum_ordinals() {
        assert_eq!(RandomType::None.ordinal(), 0);
        assert_eq!(RandomType::Gaussian.ordinal(), 1);
        assert_eq!(RandomType::Uniform.ordinal(), 2);
        assert_eq!(RandomType::LogNormal.ordinal(), 3);
        for m in [
            RandomType::None,
            RandomType::Gaussian,
            RandomType::Uniform,
            RandomType::LogNormal,
        ] {
            assert_eq!(RandomType::from_ordinal(m.ordinal()), Some(m));
        }
        assert_eq!(RandomType::from_ordinal(-1), None);
        assert_eq!(RandomType::from_ordinal(4), None);
        // `TSolutionObj.Create` default is GAUSSIAN (`Solution.pas:487`), which
        // is what `Solution::new` seeds — so `default()` must be Gaussian, not
        // the numerically-zero `None`.
        assert_eq!(RandomType::default(), RandomType::Gaussian);
    }
}
