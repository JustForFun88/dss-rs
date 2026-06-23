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
mod energymeter_registers;
mod energymeter_zones;
mod lifecycle;
mod line_fetch;
mod monitors;
mod open_close;
mod pvsystem;
mod reduce;
mod reliability;
mod solve;
mod time_series;
