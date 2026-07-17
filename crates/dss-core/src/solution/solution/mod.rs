//! Port of `Common/Solution.pas` (`TSolutionObj`): the snapshot power-flow
//! path (`Solve` → `SolveSnap` → `SolveCircuit` → `DoPFLOWsolution` →
//! `DoNormalSolution`), `Converged`, the injection machinery, `SolveSystem`,
//! `SolveZeroLoadSnapShot`, `SetVoltageBases`, `SolveDirect`; plus, since
//! Phase 5, the full `Set_Mode`, the DynaVars clock (`IncrementTime`), the
//! live `CheckControls` (control loop in [`crate::solution::controls`]) and
//! the `SolutionAlgs.pas` time-series modes (`SolveDaily`/`SolveYearly`/
//! `SolveDuty`/`SolvePeakDay`).
//!
//! Pascal reaches elements through `ActiveCircuit` pointer lists; here the
//! free functions take the `Circuit` plus a `SolveEnv` carrying the
//! element store, the parser (for bus reprocessing) and the error sink.
//!
//! Split into submodules (no behavioral change):
//! - `state` — the `SolveMode`/`ActiveY` enums, the load/algorithm/control mode
//!   constants, `SolveEnv`, the `Solution` state struct and its inherent
//!   methods, and `sys_ctx`.
//! - `set_mode` — `TSolutionObj.Set_Mode`.
//! - `power_flow` — the snapshot power-flow path (injection, `DoNormalSolution`,
//!   `SolveZeroLoadSnapShot`, the generator dQ/dV seed, `CheckControls`,
//!   `SolveSnap`, `SolveDirect`).
//! - `time_series` — the `SolutionAlgs.pas` stepping modes.
//! - `monte_carlo` — the MonteCarlo solve modes (`SolveMonte1/2/3`,
//!   `SolveMonteFault`/`PickAFault`) over the FPC RNG.
//! - `harmonics` — the harmonics solve mode (`SolveHarmonic`/`SolveHarmonicT`,
//!   the frequency sweep, `InitializeForHarmonics`).
//! - `dynamics` — the dynamics solve mode (`SolveDynamic` predictor/corrector
//!   loop, `IntegratePCStates`, `calcInitialMachineStates`).
//! - `dispatch` — the top-level `Solve` mode dispatcher and `SetVoltageBases`.

mod dispatch;
mod dynamics;
mod fault_study;
mod harmonics;
mod monte_carlo;
mod ncim;
mod power_flow;
mod set_mode;
mod state;
mod time_series;

pub use dispatch::{set_voltage_bases, solve};
pub(crate) use power_flow::solve_snap;
pub use power_flow::solve_zero_load_snapshot;
pub use set_mode::set_mode;
pub use state::{
    ADMITTANCE, ActiveY, CONTROLSOFF, CTRLSTATIC, EVENTDRIVEN, GAUSSIAN, LOGNORMAL, MULTIRATE,
    NCIM_PQ_NODE, NCIM_PV_NODE, POWERFLOW, Solution, SolveAlgorithm, SolveEnv, SolveMode,
    SolveResult, TIMEDRIVEN, UNIFORM, USEDAILY, USEDUTY, USENONE, USEYEARLY, sys_ctx,
};

pub(crate) use ncim::do_ncim_solution;

pub(crate) use dynamics::calc_initial_machine_states;
pub(crate) use harmonics::initialize_for_harmonics;
pub(crate) use power_flow::solve_circuit;
// The `_InitSnap`/`_SolveNoControl`/`_SolveDirect`/`_SolvePFlow` step-solution
// executive commands (`ExecCommands.pas:578-601`) drive these directly.
pub(crate) use power_flow::{do_pflow_solution, set_generator_disp_ref, solve_direct};
// The A-Diakoptics child-side solve stage + the coordinator generator-dQ/dV seed,
// driven by the executive coordinator loop (`exec/diakoptics/solve.rs`, WP-AD.3).
// `do_newton_solution` is the coordinator's full-system Newton (the AD `Newton`
// dispatch: official `DoNewtonSolution` has no ADiakoptics branch — Solution.pas
// :1018 — so a Newton AD deck bypasses the child stitch and solves the closed
// interconnected coordinator directly).
pub(crate) use power_flow::{do_newton_solution, set_generator_dqdv, solve_ad};
// The per-step sampling/cleanup tail shared by the time-series modes, reused by
// the A-Diakoptics time-series coordinator loop.
pub(crate) use time_series::{end_of_time_step_cleanup, sample_all_monitors_and_meters};
