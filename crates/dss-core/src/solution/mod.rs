//! Solution engine: `Solution.pas` + `Ymatrix.pas` (Phase 3 subset).
#![allow(clippy::module_inception)]

pub mod solution;
pub mod ymatrix;

pub use solution::{
    ADMITTANCE, ActiveY, CONTROLSOFF, CTRLSTATIC, NEWTONSOLVE, NORMALSOLVE, POWERFLOW, Solution,
    SolveEnv, SolveMode, SolveResult, set_voltage_bases, solve, solve_zero_load_snapshot, sys_ctx,
};
pub use ymatrix::{BuildOption, build_y_matrix, initialize_node_vbase};
