//! Exec-layer integration tests, split by theme for readability (no behavioral
//! change — see `SPLITTING_RULES.md`). The shared `Dss` query/setup helpers live in
//! [`common`]; each themed submodule owns its own group-local builders.
//!
//! `#[cfg(test)]` is carried by the single `mod tests;` declaration in the
//! parent (`exec/mod.rs`), which gates this whole subtree — no submodule needs
//! its own attribute.

mod common;

mod allocation;
mod autoadd;
mod controls;
mod distribute_uuids;
mod dynamics;
mod energymeter_registers;
mod energymeter_zones;
mod espvl_control;
mod exec_tail;
mod fault_study;
mod harmonics;
mod lifecycle;
mod line_fetch;
mod monitors;
mod open_close;
mod pvsystem;
mod reduce;
mod reliability;
mod report;
mod select;
mod solve;
mod storage;
mod time_series;
mod upfc;
mod vccs;
mod vs_converter;
