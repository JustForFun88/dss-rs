//! The object model: the Rust replacement for Pascal's `TDSSObject` /
//! `TDSSClass` / `TDSSObjectHelper` pointer-offset reflection. Properties are
//! described by per-class tables and driven by a generic parse/get engine
//! (see PORTING_PLAN.md §2.2).

pub mod arena;
pub mod base;
pub mod dss_enum;
pub mod props;
