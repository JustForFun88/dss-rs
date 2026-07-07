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
//! - `harmonics` — the harmonics solve mode (`SolveHarmonic`/`SolveHarmonicT`,
//!   the frequency sweep, `InitializeForHarmonics`).
//! - `dynamics` — the dynamics solve mode (`SolveDynamic` predictor/corrector
//!   loop, `IntegratePCStates`, `calcInitialMachineStates`).
//! - `dispatch` — the top-level `Solve` mode dispatcher and `SetVoltageBases`.

mod dispatch;
mod dynamics;
mod fault_study;
mod harmonics;
mod power_flow;
mod set_mode;
mod state;
mod time_series;

pub use dispatch::{set_voltage_bases, solve};
pub use power_flow::solve_zero_load_snapshot;
pub use set_mode::set_mode;
pub use state::{
    ADMITTANCE, ActiveY, CONTROLSOFF, CTRLSTATIC, EVENTDRIVEN, MULTIRATE, NEWTONSOLVE, NORMALSOLVE,
    POWERFLOW, Solution, SolveEnv, SolveMode, SolveResult, TIMEDRIVEN, USEDAILY, USEDUTY, USENONE,
    USEYEARLY, sys_ctx,
};

pub(crate) use dynamics::calc_initial_machine_states;
pub(crate) use harmonics::initialize_for_harmonics;
pub(crate) use power_flow::solve_circuit;
