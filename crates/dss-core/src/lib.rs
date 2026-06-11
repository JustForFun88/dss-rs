#![forbid(unsafe_code)]
//! The DSS engine: circuit model, solution algorithms, elements, and the
//! command executive. Rust port of the Pascal engine in `.inputs/dss_capi`
//! (see PORTING_PLAN.md at the repository root).

pub mod circuit;
pub mod elements;
pub mod exec;
pub mod obj;
pub mod solution;
pub mod support;
pub mod util;
