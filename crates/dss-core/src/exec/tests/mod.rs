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
mod autotrans_xfmrcode;
mod base_frequency;
mod compat_quirks;
mod controls;
mod derived_polar;
mod derived_seq;
mod derived_totals;
mod distribute_uuids;
mod dynamics;
mod element_extras;
mod energymeter_registers;
mod energymeter_zones;
mod espvl_control;
mod exec_tail;
mod fault_study;
mod force_hooks;
mod harmonics;
mod lifecycle;
mod line_fetch;
mod live_ctx;
mod make_pos_seq;
mod monitors;
mod ncim;
mod newton;
mod open_close;
mod options_timing;
mod pvsystem;
mod reduce;
mod reliability;
mod report;
mod select;
mod solve;
mod storage;
mod time_series;
mod upfc;
mod upstream_stubs;
mod vccs;
mod vs_converter;
mod windgen_usermodel;
