//! Harmonics golden gate (PHASE7_PLAN.md WP7.6; part of the former Phase-7
//! golden bucket): command-replay scenarios that pin the harmonics frequency
//! sweep — spectrum-driven load/source/DER injection at each harmonic, the
//! harmonic system Y, and the swept monitor channels — against the pinned
//! oracle, from `tests/golden/harmonics/` (one `<scenario>.json` per scenario;
//! the gate runs every file in the dir).
//!
//! Pins: converged + iteration count + node order exact, node voltages 1e-6
//! rel, the scenario Line's YPrim entry-by-entry, every element's terminal
//! currents/powers, and the per-step monitor trajectories of the sweep.
//!
//! Regenerate only manually: `python tools/golden/gen_der_lines_harmonics.py`.

mod harness;

#[test]
fn harmonics_scenarios_match_oracle() {
    harness::scenario::check_family(
        "harmonics",
        &[
            "harmonics_load_h5",
            "harmonics_load_h7",
            "harmonics_vsource",
            "harmonics_doall",
            "harmonics_doall_t",
            "harmonics_load_motor_h5",
            "harmonics_generator_h5",
            "harmonics_pvsystem_h5",
            "harmonics_storage_h5",
            "harmonics_generator_delta_h5",
        ],
    );
}
