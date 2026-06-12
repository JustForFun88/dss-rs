//! Solution engine: `Solution.pas` + `Ymatrix.pas` (Phase 3 subset).
#![allow(clippy::module_inception)]

pub mod control_queue;
pub(crate) mod controls;
pub mod event_log;
pub mod solution;
pub mod ymatrix;

pub use control_queue::{ControlActioner, ControlQueue, TimeRec};
pub use event_log::EventLog;
pub use solution::{
    ADMITTANCE, ActiveY, CONTROLSOFF, CTRLSTATIC, EVENTDRIVEN, MULTIRATE, NEWTONSOLVE, NORMALSOLVE,
    POWERFLOW, Solution, SolveEnv, SolveMode, SolveResult, TIMEDRIVEN, set_mode, set_voltage_bases,
    solve, solve_zero_load_snapshot, sys_ctx,
};
pub use ymatrix::{BuildOption, build_y_matrix, initialize_node_vbase};
