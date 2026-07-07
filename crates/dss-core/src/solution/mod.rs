//! Solution engine: `Solution.pas` + `Ymatrix.pas` (Phase 3 subset).
#![allow(clippy::module_inception)]

pub mod control_queue;
pub(crate) mod controls;
pub mod event_log;
pub(crate) mod faults;
pub(crate) mod meters;
pub(crate) mod monitors;
pub mod solution;
pub(crate) mod topology;
pub mod ymatrix;

pub use control_queue::{ControlActioner, ControlQueue, TimeRec};
pub use event_log::EventLog;
pub use solution::{
    ADMITTANCE, ActiveY, CONTROLSOFF, CTRLSTATIC, EVENTDRIVEN, MULTIRATE, NEWTONSOLVE, NORMALSOLVE,
    POWERFLOW, Solution, SolveEnv, SolveMode, SolveResult, TIMEDRIVEN, USEDAILY, USEDUTY, USENONE,
    USEYEARLY, set_mode, set_voltage_bases, solve, solve_zero_load_snapshot, sys_ctx,
};
pub(crate) use solution::{calc_initial_machine_states, initialize_for_harmonics};
pub use ymatrix::{BuildOption, build_y_matrix, initialize_node_vbase};
