//! Line-constants golden gate (PHASE7_PLAN.md WP7.1 / §1 focused gate 1; part
//! of the former Phase-7 golden bucket): command-replay scenarios that pin the
//! Carson line-constants / geometry path against the pinned oracle. Each
//! scenario builds a small circuit whose Line gets its Z/Yc from the Carson
//! engine (via `geometry=`, or `spacing=` with `wires=`/`cncable=`/`tscable=`),
//! solves once, and must match its file under `tests/golden/line_constants/`
//! (one `<scenario>.json` per scenario; the gate runs every file in the dir):
//!
//!   - line_geometry: 3-phase overhead via `geometry=` (no reduce);
//!   - line_geometry_reduce: 3 phases + a neutral, `reduce=yes` (Kron reduce);
//!   - line_spacing: 3-phase overhead via `spacing=` + `wires=`;
//!   - cable_cn: 3-phase concentric-neutral cable via `geometry=` + `cncable=`;
//!   - cable_ts: 3-phase tape-shield cable via `geometry=` + `tscable=`.
//!
//! Pins: converged + iteration count + node order exact, node voltages 1e-6 rel,
//! the **Line YPrim entry-by-entry** (the Carson Z/Yc is the math under test),
//! and every element's terminal currents/powers (voltage-scaled power floor).
//! Unlike the live corpus gate this golden is committed, so it guards the
//! geometry path offline (no oracle install needed to catch a regression).
//!
//! Regenerate only manually: `python tools/golden/gen_der_lines_harmonics.py`.

mod harness;

#[test]
fn line_constants_scenarios_match_oracle() {
    harness::scenario::check_family(
        "line_constants",
        &[
            "line_geometry",
            // UPGRADE_PLAN WP-U1.2 B2/D1: SimpleCarson De 658.5 →
            // 658.8530451057239; this golden is capi015-generated (its
            // `oracle.engine_spec == "capi015"`) and pins the upgraded Carson Z.
            "line_geometry_carson",
            "line_geometry_reduce",
            "line_spacing",
            "cable_cn",
            "cable_ts",
        ],
    );
}
