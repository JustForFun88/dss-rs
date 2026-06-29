//! Per-report formatters for `Export` (Pascal `Common/ExportResults.pas`). Each
//! takes a read view of the (solved) circuit and returns the report text/CSV; no
//! formatter mutates electrical state (PHASE8_PLAN §2.1). Coverage grows per WP
//! (WP8.1: `Counts`; WP8.2: the solution exports; WP8.3: device/meter/reliability).

mod counts;

pub use counts::export_counts;
