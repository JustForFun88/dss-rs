#![forbid(unsafe_code)]
// clippy 0.1.96+ mis-fires `collapsible_match` on the byte-faithful Pascal-port
// idiom `match prop { CONST => { if cond { side_effect } } ... _ => {} }` (the
// Pascal `case Idx of CONST: if cond then ...` dispatch). That lint targets
// nested `match`/`if let` on a destructured binding, not a boolean `if` inside an
// arm, and its autofix even drops `else` branches (e.g. line/accessors.rs
// prop_scale), so following it would change behavior. We keep the uniform `if`
// form (faithful to Pascal, consistent with the two-`if` arms the lint can't
// touch) and silence the false positive crate-wide.
#![allow(clippy::collapsible_match)]
//! The DSS engine: circuit model, solution algorithms, elements, and the
//! command executive. Rust port of the Pascal engine in `.inputs/dss_capi`
//! (see PORTING_PLAN.md at the repository root).

pub mod cim;
pub mod circuit;
pub mod diag;
pub mod elements;
pub mod exec;
pub mod obj;
pub mod report;
pub mod solution;
pub mod support;
pub mod util;
